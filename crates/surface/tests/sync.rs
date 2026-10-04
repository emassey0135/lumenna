//! Pairing and the sync service, end to end through the surface, offline.
//!
//! `Reach::LocalOnly` uses no relay and no lookup service: the devices find each other by
//! mDNS on this machine, as two devices on one network would.
#![cfg(feature = "sync")]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

use lumenna_surface::{Lumenna, PairingPrompt, Reach, SyncListener};

struct Yes {
    code: Mutex<Option<mpsc::Sender<String>>>,
    words: Mutex<Vec<String>>,
}

impl Yes {
    fn new(code: Option<mpsc::Sender<String>>) -> Arc<Self> {
        Arc::new(Self { code: Mutex::new(code), words: Mutex::default() })
    }
}

impl PairingPrompt for Yes {
    fn show_code(&self, code: String) {
        if let Some(sender) = self.code.lock().unwrap().take() {
            sender.send(code).unwrap();
        }
    }
    fn confirm(&self, words: Vec<String>) -> bool {
        *self.words.lock().unwrap() = words;
        true
    }
    fn is_cancelled(&self) -> bool {
        false
    }
}

#[derive(Default)]
struct Count(AtomicUsize);

impl SyncListener for Count {
    fn changed(&self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

fn titles(lumenna: &Lumenna) -> Vec<String> {
    lumenna.list_tasks("").unwrap().rows.into_iter().map(|row| row.title).collect()
}

fn eventually(within: Duration, check: impl Fn() -> bool) -> bool {
    let start = Instant::now();
    while start.elapsed() < within {
        if check() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    check()
}

/// Pairs two fresh stores by code, as a phone and a laptop on one network would.
fn paired() -> (tempfile::TempDir, Arc<Lumenna>, Arc<Lumenna>) {
    let dir = tempfile::tempdir().unwrap();
    let a = Arc::new(Lumenna::open_at(&dir.path().join("a")).unwrap());
    let b = Arc::new(Lumenna::open_at(&dir.path().join("b")).unwrap());
    a.add_task("written on a before pairing").unwrap();

    let (code, heard) = mpsc::channel();
    let waiting_prompt = Yes::new(Some(code));
    let waiting = {
        let (a, prompt) = (Arc::clone(&a), Arc::clone(&waiting_prompt));
        std::thread::spawn(move || {
            a.pair(None, Reach::LocalOnly, "laptop".into(), "macos".into(), prompt)
        })
    };
    let code = heard.recv_timeout(Duration::from_secs(30)).unwrap();
    let joining_prompt = Yes::new(None);
    let joined = b
        .pair(Some(code), Reach::LocalOnly, "phone".into(), "ios".into(), joining_prompt.clone())
        .unwrap();
    let waited = waiting.join().unwrap().unwrap();

    assert_eq!(joined.name, "laptop");
    assert_eq!(waited.name, "phone");
    assert_eq!(
        *waiting_prompt.words.lock().unwrap(),
        *joining_prompt.words.lock().unwrap(),
        "both devices showed the same words"
    );
    (dir, a, b)
}

#[test]
fn pairing_by_code_enrolls_both_devices_and_brings_everything_across() {
    let (_dir, a, b) = paired();
    assert_eq!(a.devices().unwrap().devices.len(), 2);
    assert_eq!(b.devices().unwrap().devices.len(), 2);
    assert!(titles(&b).contains(&"written on a before pairing".to_owned()));
}

#[test]
fn running_services_send_each_others_edits_on_without_being_asked() {
    let (_dir, a, b) = paired();
    let heard_on_a = Arc::new(Count::default());
    let heard_on_b = Arc::new(Count::default());
    let service_a = a.start_sync(Reach::LocalOnly, heard_on_a.clone()).unwrap();
    let service_b = b.start_sync(Reach::LocalOnly, heard_on_b.clone()).unwrap();
    assert!(a.sync_status().unwrap().running);

    // Written through the same connection the service uses, which `refresh` cannot see:
    // the service has to notice it from the documents themselves.
    b.add_task("written on b while both run").unwrap();
    assert!(
        eventually(Duration::from_secs(60), || titles(&a)
            .contains(&"written on b while both run".to_owned())),
        "a never received b's edit"
    );
    assert!(
        eventually(Duration::from_secs(5), || heard_on_a.0.load(Ordering::SeqCst) > 0),
        "a's listener was never told"
    );

    service_a.stop();
    service_b.stop();
    assert!(!a.endpoint_held().unwrap(), "stopping lets go of the endpoint");
}

#[test]
fn a_second_service_on_one_store_is_the_first() {
    let dir = tempfile::tempdir().unwrap();
    let a = Lumenna::open_at(&dir.path().join("a")).unwrap();
    let first = a.start_sync(Reach::LocalOnly, Arc::new(Count::default())).unwrap();
    let second = a.start_sync(Reach::LocalOnly, Arc::new(Count::default())).unwrap();
    assert!(Arc::ptr_eq(&first, &second));
    first.stop();
}

#[test]
fn another_store_on_the_same_profile_is_told_the_endpoint_is_taken() {
    let dir = tempfile::tempdir().unwrap();
    let a = Lumenna::open_at(&dir.path().join("a")).unwrap();
    let elsewhere = Lumenna::open_at(&dir.path().join("a")).unwrap();
    let service = a.start_sync(Reach::LocalOnly, Arc::new(Count::default())).unwrap();
    assert!(matches!(
        elsewhere.sync_now(Reach::LocalOnly),
        Err(lumenna_surface::LumennaError::SyncElsewhere { .. })
    ));
    service.stop();
}
