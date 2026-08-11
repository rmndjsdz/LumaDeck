use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

const CACHE_CONTROL: &str = "public, max-age=31536000, immutable";

pub struct MediaServer {
    base_url: String,
    shutdown: Arc<std::sync::atomic::AtomicBool>,
    thread: Mutex<Option<JoinHandle<()>>>,
}

impl MediaServer {
    pub fn start(root: PathBuf) -> std::io::Result<Self> {
        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        listener.set_nonblocking(true)?;
        let port = listener.local_addr()?.port();
        let shutdown = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let thread_shutdown = Arc::clone(&shutdown);
        let thread = thread::Builder::new()
            .name("lumadeck-media-server".to_string())
            .spawn(move || run(listener, root, thread_shutdown))?;

        Ok(Self {
            base_url: format!("http://127.0.0.1:{port}"),
            shutdown,
            thread: Mutex::new(Some(thread)),
        })
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }
}

impl Drop for MediaServer {
    fn drop(&mut self) {
        self.shutdown
            .store(true, std::sync::atomic::Ordering::Release);
        if let Ok(mut thread) = self.thread.lock() {
            if let Some(thread) = thread.take() {
                let _ = thread.join();
            }
        }
    }
}

fn run(listener: TcpListener, root: PathBuf, shutdown: Arc<std::sync::atomic::AtomicBool>) {
    while !shutdown.load(std::sync::atomic::Ordering::Acquire) {
        match listener.accept() {
            Ok((stream, _)) => {
                let request_root = root.clone();
                thread::spawn(move || handle_connection(stream, &request_root));
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(std::time::Duration::from_millis(10));
            }
            Err(_) => break,
        }
    }
}

fn handle_connection(mut stream: TcpStream, root: &Path) {
    let mut request = [0_u8; 8192];
    let bytes_read = match stream.read(&mut request) {
        Ok(bytes_read) => bytes_read,
        Err(_) => return,
    };
    let request = String::from_utf8_lossy(&request[..bytes_read]);
    let Some((method, target)) = request.lines().next().and_then(parse_request_line) else {
        write_error(&mut stream, 400, "Bad Request");
        return;
    };
    if method != "GET" && method != "HEAD" {
        write_error(&mut stream, 405, "Method Not Allowed");
        return;
    }

    let Some(encoded_path) = query_value(target, "path") else {
        write_error(&mut stream, 404, "Not Found");
        return;
    };
    let requested = PathBuf::from(percent_decode(encoded_path));
    let Some(path) = allowed_path(root, requested) else {
        write_error(&mut stream, 403, "Forbidden");
        return;
    };
    let Ok(bytes) = fs::read(&path) else {
        write_error(&mut stream, 404, "Not Found");
        return;
    };

    let content_type = content_type(&path);
    let headers = format!(
        "HTTP/1.1 200 OK\r\nAccess-Control-Allow-Origin: *\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: {CACHE_CONTROL}\r\nConnection: close\r\n\r\n",
        bytes.len()
    );
    if stream.write_all(headers.as_bytes()).is_ok() && method == "GET" {
        let _ = stream.write_all(&bytes);
    }
}

fn parse_request_line(line: &str) -> Option<(&str, &str)> {
    let mut parts = line.split_whitespace();
    Some((parts.next()?, parts.next()?))
}

fn query_value<'a>(target: &'a str, name: &str) -> Option<&'a str> {
    target
        .split_once('?')?
        .1
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .find_map(|(key, value)| (key == name).then_some(value))
}

fn allowed_path(root: &Path, requested: PathBuf) -> Option<PathBuf> {
    let root = root.canonicalize().ok()?;
    let requested = requested.canonicalize().ok()?;
    requested.starts_with(root).then_some(requested)
}

fn content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("avif") => "image/avif",
        Some("gif") => "image/gif",
        Some("jpeg") | Some("jpg") => "image/jpeg",
        Some("png") => "image/png",
        Some("svg") => "image/svg+xml",
        Some("webp") => "image/webp",
        Some("mp3") => "audio/mpeg",
        Some("wav") => "audio/wav",
        _ => "application/octet-stream",
    }
}

fn write_error(stream: &mut TcpStream, status: u16, reason: &str) {
    let response = format!(
        "HTTP/1.1 {status} {reason}\r\nCache-Control: no-store\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
    );
    let _ = stream.write_all(response.as_bytes());
}

fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let (Some(high), Some(low)) = (hex(bytes[index + 1]), hex(bytes[index + 2])) {
                decoded.push((high << 4) | low);
                index += 3;
                continue;
            }
        }
        decoded.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&decoded).into_owned()
}

fn hex(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{allowed_path, content_type, percent_decode};
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn decodes_encoded_paths_without_bypassing_root_checks() {
        assert_eq!(
            percent_decode("C%3A%2Fmedia%2Fcover.webp"),
            "C:/media/cover.webp"
        );
        assert_eq!(
            content_type(PathBuf::from("cover.webp").as_path()),
            "image/webp"
        );
    }

    #[test]
    fn rejects_files_outside_the_media_root() {
        let base =
            std::env::temp_dir().join(format!("lumadeck-media-server-test-{}", std::process::id()));
        let root = base.join("root");
        let outside = base.join("outside.webp");
        fs::create_dir_all(&root).expect("create test root");
        fs::write(root.join("safe.webp"), b"safe").expect("write safe asset");
        fs::write(&outside, b"outside").expect("write outside asset");

        assert!(allowed_path(&root, root.join("safe.webp")).is_some());
        assert!(allowed_path(&root, root.join("..").join("outside.webp")).is_none());

        fs::remove_dir_all(base).expect("remove test root");
    }
}
