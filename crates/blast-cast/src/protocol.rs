use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

use base64::Engine as _;

use crate::{OutputConfig, ServerKind};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const IO_TIMEOUT: Duration = Duration::from_secs(10);
const USER_AGENT: &str = concat!("BLAST-Broadcaster/", env!("CARGO_PKG_VERSION"));

#[derive(Debug)]
pub enum CastError {
    Connect(String),
    Io(std::io::Error),
    Auth,
    MountInUse,
    Rejected(String),
}

impl CastError {
    pub fn is_fatal(&self) -> bool {
        matches!(self, CastError::Auth)
    }
}

impl std::fmt::Display for CastError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CastError::Connect(e) => write!(f, "serveur injoignable ({e})"),
            CastError::Io(e) => write!(f, "connexion interrompue ({e})"),
            CastError::Auth => write!(f, "mot de passe ou identifiant refusé"),
            CastError::MountInUse => {
                write!(f, "point de montage déjà utilisé par une autre source")
            }
            CastError::Rejected(e) => write!(f, "refusé par le serveur : {e}"),
        }
    }
}

impl std::error::Error for CastError {}

impl From<std::io::Error> for CastError {
    fn from(e: std::io::Error) -> Self {
        CastError::Io(e)
    }
}

fn clean(s: &str) -> String {
    s.chars().filter(|c| *c != '\r' && *c != '\n').collect()
}

pub fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

fn connect(host: &str, port: u16) -> Result<TcpStream, CastError> {
    let addrs: Vec<_> = (host.trim(), port)
        .to_socket_addrs()
        .map_err(|e| CastError::Connect(format!("adresse {host}:{port} inconnue : {e}")))?
        .collect();
    let mut last = None;
    for addr in addrs {
        match TcpStream::connect_timeout(&addr, CONNECT_TIMEOUT) {
            Ok(s) => {
                s.set_read_timeout(Some(IO_TIMEOUT))?;
                s.set_write_timeout(Some(IO_TIMEOUT))?;
                s.set_nodelay(true)?;
                return Ok(s);
            }
            Err(e) => last = Some(e),
        }
    }
    Err(CastError::Connect(
        last.map(|e| e.to_string())
            .unwrap_or_else(|| format!("{host}:{port}")),
    ))
}

fn read_until(stream: &mut TcpStream, end: &[u8], limit: usize) -> Result<Vec<u8>, CastError> {
    let mut buf = Vec::new();
    let mut byte = [0u8; 1];
    while buf.len() < limit {
        match stream.read(&mut byte) {
            Ok(0) => break,
            Ok(_) => {
                buf.push(byte[0]);
                if buf.ends_with(end) {
                    break;
                }
            }
            Err(e) => return Err(e.into()),
        }
    }
    Ok(buf)
}

fn http_status(head: &[u8]) -> (u16, String) {
    let text = String::from_utf8_lossy(head);
    let line = text.lines().next().unwrap_or("").trim().to_string();
    let code = line
        .split_whitespace()
        .nth(1)
        .and_then(|c| c.parse().ok())
        .unwrap_or(0);
    (code, line)
}

fn basic_auth(user: &str, password: &str) -> String {
    base64::engine::general_purpose::STANDARD.encode(format!("{user}:{password}"))
}

fn audio_info(cfg: &OutputConfig, sample_rate: u32) -> String {
    format!(
        "ice-samplerate={sample_rate};ice-bitrate={};ice-channels={}",
        cfg.bitrate,
        cfg.channels()
    )
}

pub fn open_source(
    cfg: &OutputConfig,
    password: &str,
    sample_rate: u32,
) -> Result<TcpStream, CastError> {
    match cfg.kind {
        ServerKind::Icecast => open_icecast(cfg, password, sample_rate),
        ServerKind::ShoutcastV1 | ServerKind::ShoutcastV2 => open_shoutcast(cfg, password),
    }
}

fn open_icecast(
    cfg: &OutputConfig,
    password: &str,
    sample_rate: u32,
) -> Result<TcpStream, CastError> {
    let mut s = connect(&cfg.host, cfg.port)?;
    let user = if cfg.user.trim().is_empty() {
        "source"
    } else {
        cfg.user.trim()
    };
    let head = format!(
        "PUT {mount} HTTP/1.1\r\n\
         Host: {host}:{port}\r\n\
         Authorization: Basic {auth}\r\n\
         User-Agent: {USER_AGENT}\r\n\
         Content-Type: audio/mpeg\r\n\
         Ice-Public: {public}\r\n\
         Ice-Name: {name}\r\n\
         Ice-Description: {desc}\r\n\
         Ice-Genre: {genre}\r\n\
         Ice-Url: {url}\r\n\
         Ice-Audio-Info: {info}\r\n\
         Expect: 100-continue\r\n\r\n",
        mount = cfg.mount_path(),
        host = clean(&cfg.host),
        port = cfg.port,
        auth = basic_auth(user, password),
        public = u8::from(cfg.public),
        name = clean(&cfg.station),
        desc = clean(&cfg.description),
        genre = clean(&cfg.genre),
        url = clean(&cfg.url),
        info = audio_info(cfg, sample_rate),
    );
    s.write_all(head.as_bytes())?;
    let resp = read_until(&mut s, b"\r\n\r\n", 8192)?;
    let (code, line) = http_status(&resp);
    match code {
        100 | 200 => Ok(s),
        401 => Err(CastError::Auth),
        403 => {
            let body = String::from_utf8_lossy(&resp).to_lowercase();
            if body.contains("in use") || body.contains("mountpoint") {
                Err(CastError::MountInUse)
            } else {
                Err(CastError::Rejected(line))
            }
        }
        0 => Err(CastError::Rejected(
            "réponse vide (est-ce bien un serveur Icecast ?)".into(),
        )),
        _ => Err(CastError::Rejected(line)),
    }
}

fn open_shoutcast(cfg: &OutputConfig, password: &str) -> Result<TcpStream, CastError> {
    let mut s = connect(&cfg.host, cfg.port.saturating_add(1))?;
    let pass = match cfg.kind {
        ServerKind::ShoutcastV2 => format!("{}:#{}", clean(password), cfg.sid),
        _ => clean(password),
    };
    s.write_all(format!("{pass}\r\n").as_bytes())?;
    let resp = read_until(&mut s, b"\r\n", 1024)?;
    let line = String::from_utf8_lossy(&resp).trim().to_string();
    if !line.starts_with("OK") {
        return if line.to_lowercase().contains("password") || line.is_empty() {
            Err(CastError::Auth)
        } else {
            Err(CastError::Rejected(line))
        };
    }
    let head = format!(
        "icy-name:{}\r\nicy-genre:{}\r\nicy-url:{}\r\nicy-pub:{}\r\nicy-br:{}\r\ncontent-type:audio/mpeg\r\n\r\n",
        clean(&cfg.station),
        clean(&cfg.genre),
        clean(&cfg.url),
        u8::from(cfg.public),
        cfg.bitrate
    );
    s.write_all(head.as_bytes())?;
    Ok(s)
}

pub fn update_metadata(cfg: &OutputConfig, password: &str, song: &str) -> Result<(), CastError> {
    let song = percent_encode(song);
    let (path, auth) = match cfg.kind {
        ServerKind::Icecast => {
            let user = if cfg.user.trim().is_empty() {
                "source"
            } else {
                cfg.user.trim()
            };
            (
                format!(
                    "/admin/metadata?mount={}&mode=updinfo&song={song}",
                    percent_encode(&cfg.mount_path())
                ),
                Some(basic_auth(user, password)),
            )
        }
        ServerKind::ShoutcastV1 => (
            format!(
                "/admin.cgi?pass={}&mode=updinfo&song={song}",
                percent_encode(password)
            ),
            None,
        ),
        ServerKind::ShoutcastV2 => (
            format!(
                "/admin.cgi?sid={}&pass={}&mode=updinfo&song={song}",
                cfg.sid,
                percent_encode(password)
            ),
            None,
        ),
    };
    let mut s = connect(&cfg.host, cfg.port)?;
    let mut req = format!(
        "GET {path} HTTP/1.0\r\nHost: {}:{}\r\nUser-Agent: Mozilla/5.0 ({USER_AGENT})\r\n",
        clean(&cfg.host),
        cfg.port
    );
    if let Some(a) = auth {
        req.push_str(&format!("Authorization: Basic {a}\r\n"));
    }
    req.push_str("Connection: close\r\n\r\n");
    s.write_all(req.as_bytes())?;
    let resp = read_until(&mut s, b"\r\n\r\n", 8192)?;
    let (code, line) = http_status(&resp);
    match code {
        200 => Ok(()),
        401 | 403 => Err(CastError::Auth),
        _ => Err(CastError::Rejected(line)),
    }
}

pub fn test_output(
    cfg: &OutputConfig,
    password: &str,
    sample_rate: u32,
) -> Result<String, CastError> {
    let s = open_source(cfg, password, sample_rate)?;
    let _ = s.shutdown(std::net::Shutdown::Both);
    Ok(match cfg.kind {
        ServerKind::Icecast => format!(
            "Connexion acceptée par Icecast sur {}:{}{}",
            cfg.host,
            cfg.port,
            cfg.mount_path()
        ),
        k => format!(
            "Connexion acceptée par {} sur {}:{}",
            k.label(),
            cfg.host,
            cfg.port
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::thread;

    #[test]
    fn percent_encoding() {
        assert_eq!(
            percent_encode("D-Goth - Void Relay"),
            "D-Goth%20-%20Void%20Relay"
        );
        assert_eq!(percent_encode("é&="), "%C3%A9%26%3D");
    }

    fn fake_server(reply: &'static str) -> (u16, thread::JoinHandle<String>) {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        let h = thread::spawn(move || {
            let (mut s, _) = l.accept().unwrap();
            let mut buf = Vec::new();
            let mut b = [0u8; 1];
            while !buf.ends_with(b"\r\n\r\n") {
                if s.read(&mut b).unwrap() == 0 {
                    break;
                }
                buf.push(b[0]);
            }
            s.write_all(reply.as_bytes()).unwrap();
            String::from_utf8_lossy(&buf).into_owned()
        });
        (port, h)
    }

    #[test]
    fn icecast_handshake_ok() {
        let (port, h) = fake_server("HTTP/1.1 100 Continue\r\n\r\n");
        let cfg = OutputConfig {
            host: "127.0.0.1".into(),
            port,
            station: "Radio\r\nInjection".into(),
            ..Default::default()
        };
        assert!(open_source(&cfg, "hackme", 44_100).is_ok());
        let req = h.join().unwrap();
        assert!(req.starts_with("PUT /live HTTP/1.1\r\n"));
        assert!(req.contains("Authorization: Basic c291cmNlOmhhY2ttZQ==\r\n"));
        assert!(req.contains("Ice-Name: RadioInjection\r\n"));
        assert!(req.contains("ice-samplerate=44100;ice-bitrate=128;ice-channels=2"));
    }

    #[test]
    fn icecast_bad_password() {
        let (port, _h) = fake_server("HTTP/1.1 401 Authentication Required\r\n\r\n");
        let cfg = OutputConfig {
            host: "127.0.0.1".into(),
            port,
            ..Default::default()
        };
        assert!(matches!(
            open_source(&cfg, "x", 44_100),
            Err(CastError::Auth)
        ));
    }

    #[test]
    fn shoutcast_v2_sends_sid() {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let src_port = l.local_addr().unwrap().port();
        let h = thread::spawn(move || {
            let (mut s, _) = l.accept().unwrap();
            let mut line = Vec::new();
            let mut b = [0u8; 1];
            while !line.ends_with(b"\r\n") {
                s.read_exact(&mut b).unwrap();
                line.push(b[0]);
            }
            s.write_all(b"OK2\r\nicy-caps:11\r\n\r\n").unwrap();
            String::from_utf8(line).unwrap()
        });
        let cfg = OutputConfig {
            kind: ServerKind::ShoutcastV2,
            host: "127.0.0.1".into(),
            port: src_port - 1,
            sid: 3,
            ..Default::default()
        };
        assert!(open_source(&cfg, "secret", 44_100).is_ok());
        assert_eq!(h.join().unwrap(), "secret:#3\r\n");
    }

    #[test]
    fn unreachable_server() {
        let cfg = OutputConfig {
            host: "127.0.0.1".into(),
            port: 1,
            ..Default::default()
        };
        assert!(matches!(
            open_source(&cfg, "x", 44_100),
            Err(CastError::Connect(_))
        ));
    }
}
