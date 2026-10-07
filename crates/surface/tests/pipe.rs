//! The command surface at a profile's named pipe, on Windows: reached, written through,
//! read and written at once, closed to other users, and given up on stopping.

#![cfg(all(windows, feature = "rpc"))]

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::sync::mpsc::channel;
use std::time::{Duration, Instant};

use lumenna_surface::endpoint::{self, Connection};
use lumenna_surface::rpc::{Endpoint, Host};
use lumenna_surface::Lumenna;

fn serve(directory: &Path) -> Endpoint {
    let host = Host {
        process: "test".to_owned(),
        device_name: "Test".to_owned(),
        platform: "windows".to_owned(),
        sync: None,
        backups: false,
    };
    Endpoint::serve(directory.to_path_buf(), host, || true)
}

/// A profile with a store in it, served.
fn served() -> (tempfile::TempDir, Endpoint) {
    let directory = tempfile::tempdir().unwrap();
    Lumenna::open_at(directory.path()).unwrap();
    let endpoint = serve(directory.path());
    (directory, endpoint)
}

fn connect(directory: &Path) -> Connection {
    let started = Instant::now();
    loop {
        if let Some(connection) = endpoint::connect(directory) {
            return connection;
        }
        assert!(started.elapsed() < Duration::from_secs(10), "nothing answered at the pipe");
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn request(id: u32, method: &str, params: &str) -> String {
    format!("{{\"jsonrpc\":\"2.0\",\"id\":{id},\"method\":\"{method}\",\"params\":{params}}}\n")
}

#[test]
fn a_client_reaches_the_surface_at_the_pipe_and_what_it_adds_is_in_the_store() {
    let (directory, endpoint) = served();
    let mut connection = connect(directory.path());
    let mut replies = BufReader::new(connection.try_clone().unwrap());
    connection.write_all(request(1, "initialize", "{}").as_bytes()).unwrap();
    let mut line = String::new();
    replies.read_line(&mut line).unwrap();
    assert!(line.contains("\"process\":\"test\""), "{line}");
    connection.write_all(request(2, "task.add", "{\"text\":\"Water the plants\"}").as_bytes()).unwrap();
    line.clear();
    replies.read_line(&mut line).unwrap();
    assert!(line.contains("Water the plants"), "{line}");
    let titles: Vec<String> = Lumenna::open_at(directory.path()).unwrap().list_tasks("").unwrap().rows.into_iter().map(|r| r.title).collect();
    assert_eq!(titles, ["Water the plants"]);
    drop((connection, replies));
    endpoint.stop();
}

#[test]
fn one_connection_is_read_and_written_at_once_without_either_waiting_on_the_other() {
    // A blocked read on a synchronous pipe would hold up a write on it for ever: the relay
    // reads replies while it sends requests, so the pipe has to take both at once.
    let (directory, endpoint) = served();
    let connection = connect(directory.path());
    let reader = connection.try_clone().unwrap();
    let (replies, received) = channel();
    std::thread::spawn(move || {
        for line in BufReader::new(reader).lines() {
            let Ok(line) = line else { break };
            if replies.send(line).is_err() {
                break;
            }
        }
    });
    // The reader is blocked in a read before anything is written.
    std::thread::sleep(Duration::from_millis(200));
    let mut writer = connection;
    std::thread::spawn(move || {
        for id in 1..=3 {
            writer.write_all(request(id, "initialize", "{}").as_bytes()).unwrap();
        }
        // Kept open until the replies are read.
        std::thread::sleep(Duration::from_secs(10));
    });
    for _ in 0..3 {
        let line = received.recv_timeout(Duration::from_secs(10)).expect("a reply, not a read and a write waiting on each other");
        assert!(line.contains("\"process\":\"test\""), "{line}");
    }
    endpoint.stop();
}

#[test]
fn only_this_user_may_open_the_pipe() {
    use std::os::windows::io::AsRawHandle;

    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Authorization::{
        ConvertSecurityDescriptorToStringSecurityDescriptorW, GetSecurityInfo, SDDL_REVISION_1, SE_KERNEL_OBJECT,
    };
    use windows_sys::Win32::Security::{DACL_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR};

    let (directory, endpoint) = served();
    let _served = connect(directory.path());
    // Another instance, opened only to read its descriptor back.
    let pipe = std::fs::OpenOptions::new().read(true).open(endpoint::address(directory.path()));
    let pipe = match pipe {
        Ok(pipe) => pipe,
        Err(_) => {
            std::thread::sleep(Duration::from_millis(300));
            std::fs::OpenOptions::new().read(true).open(endpoint::address(directory.path())).unwrap()
        }
    };
    let mut descriptor: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
    let read = unsafe {
        GetSecurityInfo(
            pipe.as_raw_handle(),
            SE_KERNEL_OBJECT,
            DACL_SECURITY_INFORMATION,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &raw mut descriptor,
        )
    };
    assert_eq!(read, 0, "the pipe's security is readable by its user");
    let mut text: *mut u16 = std::ptr::null_mut();
    let converted = unsafe {
        ConvertSecurityDescriptorToStringSecurityDescriptorW(
            descriptor,
            SDDL_REVISION_1,
            DACL_SECURITY_INFORMATION,
            &raw mut text,
            std::ptr::null_mut(),
        )
    };
    assert_ne!(converted, 0);
    let length = (0..).take_while(|&i| unsafe { *text.add(i) } != 0).count();
    let sddl = String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(text, length) });
    unsafe {
        LocalFree(text.cast());
        LocalFree(descriptor);
    }
    let user = lumenna_surface::rpc::pipe_user().unwrap();
    // Protected from inheritance, and one entry: full access for this user, nobody else.
    assert_eq!(sddl, format!("D:P(A;;FA;;;{user})"));
    drop(pipe);
    endpoint.stop();
}

#[test]
fn stopping_gives_the_pipe_up() {
    let (directory, endpoint) = served();
    drop(connect(directory.path()));
    let started = Instant::now();
    endpoint.stop();
    assert!(started.elapsed() < Duration::from_secs(2), "stopping took {:?}", started.elapsed());
    std::thread::sleep(Duration::from_millis(100));
    assert!(endpoint::connect(directory.path()).is_none(), "nothing answers once stopped");
}
