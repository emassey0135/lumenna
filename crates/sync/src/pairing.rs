//! Pairing: confirming a new device by comparing words.
//!
//! After the connection is up, **each side derives three words from a secret only the two ends
//! of that one connection share, both show them, and a person confirms on both that they
//! match.** Matching words mean nobody is in the middle: someone interposed would hold two
//! connections with two different secrets, and so two different lists of words. This is the
//! Signal safety-number and Bluetooth numeric-comparison pattern, and it needs nothing typed
//! and nothing dictated — three words compared by ear, the same on a watch as on a desktop.
//!
//! # Why short words are safe here
//!
//! Three words from the PGP lists are 24 bits. That would be weak if an attacker could try
//! many candidates offline and keep the one that matched. Two things stop that:
//!
//! - The secret comes from the TLS key exporter of the connection (RFC 5705), so each side's
//!   value is fixed by a handshake neither party controls alone.
//! - On top of it, a **commit-then-reveal** exchange of nonces: the dialler commits to its
//!   nonce before seeing the other's, and only then reveals it. Neither side — nor anyone in
//!   between — can choose its contribution after seeing the other's, so an attacker gets one
//!   guess per pairing attempt at a 1 in 16,777,216 chance, and a person sees each attempt.
//!
//! # The words
//!
//! The PGP word list, which magic-wormhole also uses: two lists of 256, alternating, chosen
//! to be told apart over a poor telephone line. Here that means through a speech synthesiser
//! and on a braille display, which is the same property.

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncWrite};

use crate::error::{Result, SyncError};
use crate::framing::{read_frame, write_frame};

/// How many words a person compares.
pub const WORDS: usize = 3;

/// Which end of the connection this is. The dialler commits first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// This side opened the connection.
    Dialled,
    /// This side answered it.
    Answered,
}

/// Who a device is, as it introduces itself once both sides have confirmed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Identity {
    /// Its device key, as the 64 hex digits Iroh writes.
    pub node_id: String,
    /// What its owner calls it.
    pub name: String,
    /// What it runs: `linux`, `macos`, `windows`, `btspeak`.
    pub platform: String,
    /// The stored-format version it runs. A build from before versions sends none, which
    /// reads as zero.
    #[serde(default)]
    pub schema: u32,
}

/// Exchanges nonces with the commit-then-reveal order and returns the words for this
/// connection.
///
/// `channel_secret` must be the same on both sides of one connection and different on any
/// other — the TLS exporter output of the connection.
///
/// # Errors
///
/// If the stream fails, or the dialler's revealed nonce does not match its commitment, which
/// only tampering produces.
pub async fn compare<R, W>(
    role: Role,
    reader: &mut R,
    writer: &mut W,
    channel_secret: &[u8; 32],
) -> Result<Vec<String>>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let (dialler, answerer) = match role {
        Role::Dialled => {
            let mine = nonce()?;
            write_frame(writer, commitment(&mine).as_bytes()).await?;
            let theirs = read_nonce(reader).await?;
            write_frame(writer, &mine).await?;
            (mine, theirs)
        }
        Role::Answered => {
            let committed = read_frame(reader).await?;
            let mine = nonce()?;
            write_frame(writer, &mine).await?;
            let theirs = read_nonce(reader).await?;
            if commitment(&theirs).as_bytes().as_slice() != committed.as_slice() {
                return Err(SyncError::Protocol(
                    "its nonce does not match what it committed to, which only interference \
                     produces; nothing was paired"
                        .to_owned(),
                ));
            }
            (theirs, mine)
        }
    };
    Ok(words(channel_secret, &dialler, &answerer))
}

/// Exchanges each side's decision, and, if both agreed, each side's identity.
///
/// Both sides always send, whatever they decided, so a "no" on either device ends the pairing
/// on both with a sentence saying which one said no.
///
/// # Errors
///
/// [`SyncError::NotPaired`] if either person said the words did not match; a network or
/// protocol error otherwise.
pub async fn decide<R, W>(
    reader: &mut R,
    writer: &mut W,
    accepted: bool,
    me: &Identity,
) -> Result<Identity>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    #[derive(Serialize, Deserialize)]
    struct Decision {
        accepted: bool,
        identity: Option<Identity>,
    }
    let mine = Decision { accepted, identity: accepted.then(|| me.clone()) };
    let encoded = serde_json::to_vec(&mine)
        .map_err(|e| SyncError::Protocol(format!("could not write a decision: {e}")))?;
    write_frame(writer, &encoded).await?;
    let theirs: Decision = serde_json::from_slice(&read_frame(reader).await?)
        .map_err(|e| SyncError::Protocol(format!("a decision that does not read: {e}")))?;

    if !accepted {
        return Err(SyncError::NotPaired(
            "you said the words did not match, so nothing was paired".to_owned(),
        ));
    }
    if !theirs.accepted {
        return Err(SyncError::NotPaired(
            "the other device said the words did not match, so nothing was paired".to_owned(),
        ));
    }
    let identity = theirs
        .identity
        .ok_or_else(|| SyncError::Protocol("it agreed but did not say who it is".to_owned()))?;
    if identity.node_id.parse::<lumenna_core::id::NodeId>().is_err() {
        return Err(SyncError::Protocol(format!("'{}' is not a device key", identity.node_id)));
    }
    Ok(identity)
}

/// The words for a connection secret and the two nonces.
#[must_use]
pub fn words(channel_secret: &[u8; 32], dialler: &[u8; 32], answerer: &[u8; 32]) -> Vec<String> {
    let mut input = Vec::with_capacity(96);
    input.extend_from_slice(channel_secret);
    input.extend_from_slice(dialler);
    input.extend_from_slice(answerer);
    let digest = blake3::derive_key("lumenna pairing words v0", &input);
    pgp_words::to_words(&digest[..WORDS]).into_iter().map(ToOwned::to_owned).collect()
}

fn commitment(nonce: &[u8; 32]) -> blake3::Hash {
    blake3::keyed_hash(blake3::hash(b"lumenna pairing commitment v0").as_bytes(), nonce)
}

fn nonce() -> Result<[u8; 32]> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes)
        .map_err(|e| SyncError::Protocol(format!("no randomness available: {e}")))?;
    Ok(bytes)
}

async fn read_nonce<R: AsyncRead + Unpin>(reader: &mut R) -> Result<[u8; 32]> {
    read_frame(reader)
        .await?
        .try_into()
        .map_err(|_| SyncError::Protocol("a nonce of the wrong length".to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity(name: &str) -> Identity {
        Identity { node_id: "ab".repeat(32), name: name.to_owned(), platform: "linux".to_owned(), schema: 1 }
    }

    async fn both<A, B, FA, FB>(a: A, b: B) -> (FA, FB)
    where
        A: std::future::Future<Output = FA> + Send + 'static,
        B: std::future::Future<Output = FB> + Send + 'static,
        FA: Send + 'static,
        FB: Send + 'static,
    {
        let a = tokio::spawn(a);
        let b = tokio::spawn(b);
        (a.await.unwrap(), b.await.unwrap())
    }

    #[tokio::test]
    async fn both_ends_of_one_connection_see_the_same_three_words() {
        let (left, right) = tokio::io::duplex(1024);
        let secret = [7u8; 32];
        let (a, b) = both(
            async move {
                let (mut r, mut w) = tokio::io::split(left);
                compare(Role::Dialled, &mut r, &mut w, &secret).await.unwrap()
            },
            async move {
                let (mut r, mut w) = tokio::io::split(right);
                compare(Role::Answered, &mut r, &mut w, &secret).await.unwrap()
            },
        )
        .await;
        assert_eq!(a, b);
        assert_eq!(a.len(), WORDS);
    }

    #[tokio::test]
    async fn someone_in_the_middle_holds_two_secrets_and_so_two_word_lists() {
        // The attacker has one connection to each side, with a different exporter secret on
        // each. Each side's words come from its own connection.
        let (left, right) = tokio::io::duplex(1024);
        let (a, b) = both(
            async move {
                let (mut r, mut w) = tokio::io::split(left);
                compare(Role::Dialled, &mut r, &mut w, &[1u8; 32]).await.unwrap()
            },
            async move {
                let (mut r, mut w) = tokio::io::split(right);
                compare(Role::Answered, &mut r, &mut w, &[2u8; 32]).await.unwrap()
            },
        )
        .await;
        assert_ne!(a, b);
    }

    #[tokio::test]
    async fn a_reveal_that_does_not_match_the_commitment_is_refused() {
        let (left, right) = tokio::io::duplex(1024);
        let cheat = async move {
            let (mut r, mut w) = tokio::io::split(left);
            write_frame(&mut w, commitment(&[1u8; 32]).as_bytes()).await.unwrap();
            let _theirs = read_frame(&mut r).await.unwrap();
            // Reveals a different nonce from the one it committed to.
            write_frame(&mut w, &[2u8; 32]).await.unwrap();
        };
        let honest = async move {
            let (mut r, mut w) = tokio::io::split(right);
            compare(Role::Answered, &mut r, &mut w, &[0u8; 32]).await
        };
        let ((), result) = both(cheat, honest).await;
        assert!(matches!(result, Err(SyncError::Protocol(m)) if m.contains("committed")));
    }

    #[tokio::test]
    async fn a_no_on_either_side_ends_it_on_both_and_says_which() {
        let (left, right) = tokio::io::duplex(1024);
        let (a, b) = both(
            async move {
                let (mut r, mut w) = tokio::io::split(left);
                decide(&mut r, &mut w, true, &identity("laptop")).await
            },
            async move {
                let (mut r, mut w) = tokio::io::split(right);
                decide(&mut r, &mut w, false, &identity("phone")).await
            },
        )
        .await;
        assert!(matches!(a, Err(SyncError::NotPaired(m)) if m.contains("other device")));
        assert!(matches!(b, Err(SyncError::NotPaired(m)) if m.starts_with("you said")));
    }

    #[tokio::test]
    async fn two_yeses_exchange_identities() {
        let (left, right) = tokio::io::duplex(1024);
        let (a, b) = both(
            async move {
                let (mut r, mut w) = tokio::io::split(left);
                decide(&mut r, &mut w, true, &identity("laptop")).await.unwrap()
            },
            async move {
                let (mut r, mut w) = tokio::io::split(right);
                decide(&mut r, &mut w, true, &identity("phone")).await.unwrap()
            },
        )
        .await;
        assert_eq!(a.name, "phone");
        assert_eq!(b.name, "laptop");
    }

    #[test]
    fn words_come_from_the_pgp_lists_and_differ_with_any_input() {
        let base = words(&[0; 32], &[1; 32], &[2; 32]);
        assert_eq!(base.len(), 3);
        assert_ne!(base, words(&[0; 32], &[1; 32], &[3; 32]));
        assert_ne!(base, words(&[9; 32], &[1; 32], &[2; 32]));
        assert!(pgp_words::to_bytes(&base).is_some(), "{base:?}");
    }
}
