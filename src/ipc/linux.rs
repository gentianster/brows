use anyhow::{bail, Context, Result};
use fs2::FileExt;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::{FileTypeExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

const MAX_MESSAGE: usize = 16 * 1024;
const ACK: &str = "BROWS-OK\n";
const OPEN: &str = "BROWS-OPEN ";
const EXIT: &str = "BROWS-EXIT";

pub enum Request {
    Open(String),
    Exit,
}

pub struct PipeServer {
    listener: UnixListener,
    socket_path: PathBuf,
    _lock: File,
}

impl PipeServer {
    pub fn accept(&self) -> Option<Request> {
        let (mut client, _) = self.listener.accept().ok()?;
        if !same_user(&client) {
            return None;
        }
        client.set_read_timeout(Some(Duration::from_secs(2))).ok()?;
        client
            .set_write_timeout(Some(Duration::from_secs(2)))
            .ok()?;
        let line = read_message(&client)?;
        let request = parse_request(&line)?;
        client.write_all(ACK.as_bytes()).ok()?;
        Some(request)
    }
}

impl Drop for PipeServer {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.socket_path);
    }
}

fn owned_private_directory(path: &Path) -> Result<()> {
    let meta = std::fs::symlink_metadata(path)?;
    if !meta.is_dir() || meta.uid() != unsafe { libc::geteuid() } || meta.mode() & 0o077 != 0 {
        bail!(
            "{} must be a private directory owned by the current user",
            path.display()
        );
    }
    Ok(())
}

fn runtime_base() -> Result<PathBuf> {
    let path = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .context("XDG_RUNTIME_DIR is required for the resident picker")?;
    if !path.is_absolute() {
        bail!("XDG_RUNTIME_DIR must be absolute");
    }
    owned_private_directory(&path)?;
    Ok(path)
}

pub fn try_bind() -> Result<Option<PipeServer>> {
    try_bind_at(&runtime_base()?)
}

fn try_bind_at(base: &Path) -> Result<Option<PipeServer>> {
    owned_private_directory(base)?;
    let directory = base.join("brows");
    use std::os::unix::fs::DirBuilderExt;
    let mut builder = std::fs::DirBuilder::new();
    builder.mode(0o700);
    if let Err(error) = builder.create(&directory) {
        if error.kind() != std::io::ErrorKind::AlreadyExists {
            return Err(error.into());
        }
    }
    owned_private_directory(&directory)?;
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(directory.join("resident.lock"))?;
    let meta = lock.metadata()?;
    if !meta.is_file() || meta.uid() != unsafe { libc::geteuid() } || meta.mode() & 0o077 != 0 {
        bail!("resident.lock must be a private regular file owned by the current user");
    }
    match lock.try_lock_exclusive() {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => return Ok(None),
        Err(error) => return Err(error.into()),
    }
    let socket_path = directory.join("resident.sock");
    match std::fs::symlink_metadata(&socket_path) {
        Ok(meta) => {
            if !meta.file_type().is_socket() || meta.uid() != unsafe { libc::geteuid() } {
                bail!("refusing to remove a non-owned or non-socket IPC path");
            }
            // A pathname alone cannot tell whether its server is alive after a crash.
            // The exclusive lifetime lock prevents removing a live resident's socket.
            std::fs::remove_file(&socket_path)?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    let listener = UnixListener::bind(&socket_path)?;
    std::fs::set_permissions(&socket_path, std::fs::Permissions::from_mode(0o600))?;
    Ok(Some(PipeServer {
        listener,
        socket_path,
        _lock: lock,
    }))
}

fn same_user(stream: &UnixStream) -> bool {
    let mut credentials: libc::ucred = unsafe { std::mem::zeroed() };
    let mut size = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
    unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            &mut credentials as *mut _ as *mut libc::c_void,
            &mut size,
        ) == 0
            && credentials.uid == libc::geteuid()
    }
}

fn read_message(stream: &UnixStream) -> Option<String> {
    let mut bytes = Vec::new();
    BufReader::new(stream)
        .take((MAX_MESSAGE + 1) as u64)
        .read_until(b'\n', &mut bytes)
        .ok()?;
    if bytes.len() > MAX_MESSAGE || bytes.pop()? != b'\n' {
        return None;
    }
    String::from_utf8(bytes).ok()
}

fn parse_request(line: &str) -> Option<Request> {
    if line == EXIT {
        return Some(Request::Exit);
    }
    let url = line.strip_prefix(OPEN)?;
    super::is_web_url(url).then(|| Request::Open(url.to_owned()))
}

pub fn send_open(url: &str) -> Result<bool> {
    if !super::is_web_url(url) {
        bail!("only complete HTTP and HTTPS URLs can be forwarded");
    }
    let directory = runtime_base()?.join("brows");
    match std::fs::symlink_metadata(&directory) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.into()),
        Ok(_) => owned_private_directory(&directory)?,
    }
    send_open_to(&directory.join("resident.sock"), &format!("{OPEN}{url}"))
}

fn send_open_to(path: &Path, line: &str) -> Result<bool> {
    match send_attempt(path, line) {
        SendOutcome::Delivered => Ok(true),
        SendOutcome::NotReady => Ok(false),
        // A boolean failure would let open_url retry a URL the resident already accepted.
        SendOutcome::Failed => {
            bail!("resident URL delivery was not acknowledged; delivery may have occurred")
        }
    }
}

pub fn send_open_wait(url: &str) -> Result<bool> {
    if !super::is_web_url(url) {
        bail!("only complete HTTP and HTTPS URLs can be forwarded");
    }
    let base = runtime_base()?;
    let directory = base.join("brows");
    owned_private_directory(&directory)?;
    send_wait_to(&directory.join("resident.sock"), &format!("{OPEN}{url}"))
}

pub fn send_exit() -> bool {
    send(EXIT)
}

fn send(line: &str) -> bool {
    let Ok(base) = runtime_base() else {
        return false;
    };
    let directory = base.join("brows");
    if owned_private_directory(&directory).is_err() {
        return false;
    }
    send_to(&directory.join("resident.sock"), line)
}

fn send_to(path: &Path, line: &str) -> bool {
    matches!(send_attempt(path, line), SendOutcome::Delivered)
}

enum SendOutcome {
    Delivered,
    NotReady,
    Failed,
}

fn send_wait_to(path: &Path, line: &str) -> Result<bool> {
    super::retry_not_ready(|| send_open_to(path, line), Duration::from_secs(5))
}

fn send_attempt(path: &Path, line: &str) -> SendOutcome {
    if line.len() + 1 > MAX_MESSAGE {
        return SendOutcome::Failed;
    }
    let mut stream = match UnixStream::connect(path) {
        Ok(stream) => stream,
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused
            ) =>
        {
            return SendOutcome::NotReady;
        }
        Err(_) => return SendOutcome::Failed,
    };
    if !same_user(&stream) {
        return SendOutcome::Failed;
    }
    // The first window and font setup can still be running when another process connects.
    // A short read timeout would turn that successful connection into a lost URL.
    if stream
        .set_read_timeout(Some(Duration::from_secs(30)))
        .is_err()
        || stream
            .set_write_timeout(Some(Duration::from_secs(2)))
            .is_err()
        || stream.write_all(format!("{line}\n").as_bytes()).is_err()
    {
        return SendOutcome::Failed;
    }
    if read_message(&stream).is_some_and(|message| message == ACK.trim_end()) {
        SendOutcome::Delivered
    } else {
        // Retrying after a completed write could enqueue the same URL twice.
        SendOutcome::Failed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_one_resident_owns_the_socket_and_a_crashed_socket_is_recovered() {
        let root = tempfile::tempdir().unwrap();
        std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        let first = try_bind_at(root.path()).unwrap().unwrap();
        assert!(try_bind_at(root.path()).unwrap().is_none());
        let path = first.socket_path.clone();
        drop(first);
        let stale = UnixListener::bind(&path).unwrap();
        drop(stale);
        assert!(try_bind_at(root.path()).unwrap().is_some());
    }

    #[test]
    fn concurrent_clients_deliver_every_distinct_url() {
        let root = tempfile::tempdir().unwrap();
        std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        let server = try_bind_at(root.path()).unwrap().unwrap();
        let path = server.socket_path.clone();
        let listener = std::thread::spawn(move || {
            let mut received = Vec::new();
            while received.len() < 24 {
                if let Some(Request::Open(url)) = server.accept() {
                    received.push(url);
                }
            }
            received.sort();
            received
        });
        let clients: Vec<_> = (0..24)
            .map(|i| {
                let path = path.clone();
                std::thread::spawn(move || {
                    let url = format!("https://example.invalid/{i}");
                    assert!(send_to(&path, &format!("{OPEN}{url}")));
                    url
                })
            })
            .collect();
        let mut expected: Vec<_> = clients
            .into_iter()
            .map(|thread| thread.join().unwrap())
            .collect();
        expected.sort();
        assert_eq!(listener.join().unwrap(), expected);
    }

    #[test]
    fn client_waits_for_the_lock_owner_to_finish_creating_its_socket() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("starting.sock");
        let client_path = path.clone();
        let client = std::thread::spawn(move || {
            send_wait_to(&client_path, "BROWS-OPEN https://example.invalid/startup")
        });
        std::thread::sleep(Duration::from_millis(80));
        let listener = UnixListener::bind(&path).unwrap();
        let (mut stream, _) = listener.accept().unwrap();
        assert_eq!(
            read_message(&stream).as_deref(),
            Some("BROWS-OPEN https://example.invalid/startup")
        );
        stream.write_all(ACK.as_bytes()).unwrap();
        assert!(client.join().unwrap().unwrap());
    }

    #[test]
    fn accepted_url_without_ack_is_an_error_instead_of_an_unconnected_resident() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("missing-ack.sock");
        let listener = UnixListener::bind(&path).unwrap();
        let server = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            read_message(&stream).unwrap()
        });
        assert!(send_wait_to(&path, "BROWS-OPEN https://example.invalid/accepted").is_err());
        assert_eq!(
            server.join().unwrap(),
            "BROWS-OPEN https://example.invalid/accepted"
        );
        assert!(!send_open_to(
            &root.path().join("absent.sock"),
            "BROWS-OPEN https://example.invalid/absent"
        )
        .unwrap());
    }

    #[test]
    fn refuses_non_socket_stale_paths_and_malformed_requests() {
        let root = tempfile::tempdir().unwrap();
        std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        std::fs::create_dir(root.path().join("brows")).unwrap();
        std::fs::set_permissions(
            root.path().join("brows"),
            std::fs::Permissions::from_mode(0o700),
        )
        .unwrap();
        std::fs::write(root.path().join("brows/resident.sock"), "keep").unwrap();
        assert!(try_bind_at(root.path()).is_err());
        assert_eq!(
            std::fs::read_to_string(root.path().join("brows/resident.sock")).unwrap(),
            "keep"
        );
        assert!(parse_request("BROWS-OPEN httpwhatever").is_none());
        assert!(parse_request("BROWS-EXIT extra").is_none());
        assert!(parse_request("BROWS-OPEN https://example.invalid").is_some());
    }
}
