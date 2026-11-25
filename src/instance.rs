//! One browser per user. A second launch (typically a link or file opened from another app,
//! once poof is the default browser) hands its address to the running browser and exits.
//!
//! The running browser listens on a loopback port and publishes the port and a random token
//! in `instance.json`. Only a message that starts with that token is accepted, so a web page
//! that happens to reach the port cannot open tabs.

use crate::UserEvent;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
    net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream},
    path::Path,
    sync::OnceLock,
    time::Duration,
};
use tao::event_loop::EventLoopProxy;

const LOCK_FILE: &str = "instance.json";
/// How long a launch waits for the running browser before deciding it is gone.
const FORWARD_TIMEOUT: Duration = Duration::from_millis(800);
/// How long the running browser waits for a client to finish its message.
const READ_TIMEOUT: Duration = Duration::from_secs(1);
/// A message is two short lines; anything longer is not ours.
const MAX_MESSAGE_BYTES: u64 = 8192;

/// This process's token once it owns the lock, so only its own lock file is removed on exit.
static OWN_TOKEN: OnceLock<String> = OnceLock::new();

#[derive(Serialize, Deserialize)]
struct Lock {
    port: u16,
    token: String,
}

/// The outcome of starting up.
pub enum Claim {
    /// A running browser took the address; this process should exit.
    Forwarded,
    /// This process is the browser. The listener lets later launches reach it.
    Primary(Option<Listener>),
}

pub struct Listener {
    socket: TcpListener,
    token: String,
}

/// Hands `target` to the running browser, or becomes the running browser.
pub fn claim(root: &Path, target: Option<&str>) -> Claim {
    if let Some(lock) = read_lock(root) {
        if forward(&lock, target) {
            return Claim::Forwarded;
        }
    }
    match Listener::bind(root) {
        Ok(listener) => Claim::Primary(Some(listener)),
        Err(error) => {
            log::warn!("single-instance listener unavailable: {error}");
            Claim::Primary(None)
        }
    }
}

/// Removes the lock file when the browser closes.
pub fn release(root: &Path) {
    if let (Some(own), Some(lock)) = (OWN_TOKEN.get(), read_lock(root)) {
        if lock.token == *own {
            let _ = fs::remove_file(root.join(LOCK_FILE));
        }
    }
}

fn read_lock(root: &Path) -> Option<Lock> {
    let text = fs::read_to_string(root.join(LOCK_FILE)).ok()?;
    serde_json::from_str(&text).ok()
}

/// Sends the address to the running browser; true when it acknowledged.
fn forward(lock: &Lock, target: Option<&str>) -> bool {
    let address = SocketAddr::from((Ipv4Addr::LOCALHOST, lock.port));
    let Ok(mut stream) = TcpStream::connect_timeout(&address, FORWARD_TIMEOUT) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(FORWARD_TIMEOUT));
    let _ = stream.set_write_timeout(Some(FORWARD_TIMEOUT));
    allow_foreground_window();
    let message = format!("{}\n{}\n", lock.token, target.unwrap_or(""));
    if stream.write_all(message.as_bytes()).is_err() {
        return false;
    }
    let mut reply = String::new();
    BufReader::new(&stream).read_line(&mut reply).is_ok() && reply.trim() == "ok"
}

/// Lets the browser that receives the address bring its window to the front: Windows only
/// allows that for the process the user just interacted with, or one it hands the right to.
#[cfg(windows)]
fn allow_foreground_window() {
    use windows::Win32::UI::WindowsAndMessaging::{AllowSetForegroundWindow, ASFW_ANY};
    unsafe {
        let _ = AllowSetForegroundWindow(ASFW_ANY);
    }
}

#[cfg(not(windows))]
fn allow_foreground_window() {}

impl Listener {
    fn bind(root: &Path) -> std::io::Result<Self> {
        let socket = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        let port = socket.local_addr()?.port();
        let token = crate::state::new_id();
        let lock = Lock {
            port,
            token: token.clone(),
        };
        fs::write(root.join(LOCK_FILE), serde_json::to_string(&lock)?)?;
        let _ = OWN_TOKEN.set(token.clone());
        Ok(Self { socket, token })
    }

    /// Starts accepting launches. The thread blocks in `accept`, so it costs nothing while idle.
    pub fn spawn(self, proxy: EventLoopProxy<UserEvent>) {
        let spawned = std::thread::Builder::new()
            .name("instance".into())
            .spawn(move || {
                for stream in self.socket.incoming().flatten() {
                    serve(stream, &self.token, &proxy);
                }
            });
        if let Err(error) = spawned {
            log::warn!("could not start the single-instance listener: {error}");
        }
    }
}

/// Reads one message; if it carries the right token, acknowledges it and wakes the browser.
fn serve(mut stream: TcpStream, token: &str, proxy: &EventLoopProxy<UserEvent>) {
    let _ = stream.set_read_timeout(Some(READ_TIMEOUT));
    let _ = stream.set_write_timeout(Some(READ_TIMEOUT));
    let mut lines = BufReader::new((&stream).take(MAX_MESSAGE_BYTES));
    let (mut sent_token, mut url) = (String::new(), String::new());
    if lines.read_line(&mut sent_token).is_err() || lines.read_line(&mut url).is_err() {
        return;
    }
    if !same_secret(sent_token.trim_end(), token) {
        return;
    }
    let url = url.trim();
    let url = (!url.is_empty()).then(|| url.to_string());
    let _ = stream.write_all(b"ok\n");
    let _ = proxy.send_event(UserEvent::External { url });
}

/// Compares without stopping at the first difference.
fn same_secret(a: &str, b: &str) -> bool {
    a.len() == b.len()
        && a.bytes()
            .zip(b.bytes())
            .fold(0u8, |diff, (x, y)| diff | (x ^ y))
            == 0
}

/// The address or file this launch was asked to open, from the command line.
pub fn startup_target() -> Option<String> {
    std::env::args_os()
        .skip(1)
        .filter_map(|arg| arg.into_string().ok())
        .find_map(|arg| target_from_arg(&arg))
}

/// A link (`http`, `https`, `file`) or the path of an existing file, as an address.
/// Flags and anything else are ignored.
pub fn target_from_arg(arg: &str) -> Option<String> {
    let arg = arg.trim().trim_matches('"');
    if arg.is_empty() || arg.starts_with('-') {
        return None;
    }
    let lower = arg.to_ascii_lowercase();
    if ["http://", "https://", "file:"]
        .iter()
        .any(|scheme| lower.starts_with(scheme))
    {
        return Some(arg.to_string());
    }
    let path = Path::new(arg);
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().ok()?.join(path)
    };
    if !absolute.is_file() {
        return None;
    }
    url::Url::from_file_path(&absolute)
        .ok()
        .map(|address| address.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_pass_through_and_flags_are_ignored() {
        assert_eq!(
            target_from_arg("https://example.com/a?b=1").as_deref(),
            Some("https://example.com/a?b=1")
        );
        assert_eq!(
            target_from_arg("\"HTTP://Example.com\"").as_deref(),
            Some("HTTP://Example.com")
        );
        assert_eq!(target_from_arg("--new-window"), None);
        assert_eq!(target_from_arg("-x"), None);
        assert_eq!(target_from_arg(""), None);
        assert_eq!(target_from_arg("example.com"), None);
        assert_eq!(target_from_arg("javascript:alert(1)"), None);
    }

    #[test]
    fn existing_files_become_file_addresses() {
        let file = std::env::temp_dir().join(format!("poof-open-{}.html", crate::state::new_id()));
        fs::write(&file, "<p>x</p>").unwrap();
        let address = target_from_arg(&file.to_string_lossy()).unwrap();
        assert!(address.starts_with("file:///"));
        assert!(address.ends_with(".html"));
        let _ = fs::remove_file(&file);
        assert_eq!(target_from_arg(&file.to_string_lossy()), None);
    }

    #[test]
    fn secrets_must_match_exactly() {
        assert!(same_secret("abc", "abc"));
        assert!(!same_secret("abc", "abd"));
        assert!(!same_secret("abc", "ab"));
    }

    #[test]
    fn a_launch_reaches_the_running_browser_only_with_its_token() {
        let root = std::env::temp_dir().join(format!("poof-inst-{}", crate::state::new_id()));
        fs::create_dir_all(&root).unwrap();
        let listener = Listener::bind(&root).unwrap();
        let lock = read_lock(&root).unwrap();
        let socket = listener.socket.try_clone().unwrap();
        let token = listener.token.clone();
        // Answer exactly one client without needing an event loop.
        let server = std::thread::spawn(move || {
            let (mut stream, _) = socket.accept().unwrap();
            let mut lines = BufReader::new(&stream);
            let (mut t, mut u) = (String::new(), String::new());
            lines.read_line(&mut t).unwrap();
            lines.read_line(&mut u).unwrap();
            if same_secret(t.trim_end(), &token) {
                stream.write_all(b"ok\n").unwrap();
            }
            u.trim().to_string()
        });
        assert!(forward(&lock, Some("https://example.com/")));
        assert_eq!(server.join().unwrap(), "https://example.com/");
        let _ = fs::remove_dir_all(&root);
    }
}
