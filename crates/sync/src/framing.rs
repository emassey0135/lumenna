//! Length-prefixed messages over a byte stream.
//!
//! A four-byte big-endian length and then that many bytes. A zero length is a message too:
//! the sync session uses it for "nothing to send this round", which an Automerge sync message
//! never is.

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::error::{Result, SyncError};

/// The largest message either side will accept. Far above any sync message a task store
/// produces, and low enough that a confused or hostile peer cannot make this side allocate
/// without limit.
pub(crate) const MAX_FRAME: usize = 64 * 1024 * 1024;

pub(crate) async fn write_frame<W: AsyncWrite + Unpin>(writer: &mut W, bytes: &[u8]) -> Result<()> {
    let length = u32::try_from(bytes.len())
        .ok()
        .filter(|n| (*n as usize) <= MAX_FRAME)
        .ok_or_else(|| SyncError::Protocol("a message too large to send".to_owned()))?;
    writer.write_all(&length.to_be_bytes()).await?;
    writer.write_all(bytes).await?;
    writer.flush().await?;
    Ok(())
}

pub(crate) async fn read_frame<R: AsyncRead + Unpin>(reader: &mut R) -> Result<Vec<u8>> {
    let mut length = [0u8; 4];
    reader.read_exact(&mut length).await?;
    let length = u32::from_be_bytes(length) as usize;
    if length > MAX_FRAME {
        return Err(SyncError::Protocol(format!("a message of {length} bytes, over the limit")));
    }
    let mut bytes = vec![0u8; length];
    reader.read_exact(&mut bytes).await?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn frames_round_trip_including_the_empty_one() {
        let (mut a, mut b) = tokio::io::duplex(1024);
        write_frame(&mut a, b"hello").await.unwrap();
        write_frame(&mut a, b"").await.unwrap();
        assert_eq!(read_frame(&mut b).await.unwrap(), b"hello");
        assert!(read_frame(&mut b).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn an_oversized_length_is_refused_before_allocating() {
        let (mut a, mut b) = tokio::io::duplex(64);
        tokio::io::AsyncWriteExt::write_all(&mut a, &u32::MAX.to_be_bytes()).await.unwrap();
        assert!(matches!(read_frame(&mut b).await, Err(SyncError::Protocol(_))));
    }
}
