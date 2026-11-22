use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crossbeam_channel::{bounded, unbounded, Receiver, RecvTimeoutError, Sender};
use serde::Serialize;

use crate::encoder::Mp3Encoder;
use crate::protocol::{open_source, update_metadata};
use crate::OutputConfig;

// [CAST-09]
const RETRY_DELAY: Duration = Duration::from_secs(5);
// [CAST-10]
const PCM_QUEUE: usize = 128;

// [CAST-11]
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "state", content = "detail", rename_all = "snake_case")]
pub enum OutputStatus {
    Off,
    Connecting,
    Live,
    Retrying(String),
    Error(String),
}

// [CAST-12]
#[derive(Debug, Clone)]
pub struct StatusEvent {
    pub id: String,
    pub status: OutputStatus,
}

// [CAST-13]
struct Handle {
    id: String,
    pcm: Sender<Arc<[f32]>>,
    meta: Sender<String>,
    stop: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}

// [CAST-14]
pub struct Caster {
    sample_rate: u32,
    outputs: Vec<Handle>,
    status_tx: Sender<StatusEvent>,
    status_rx: Receiver<StatusEvent>,
    metadata: String,
}

// [CAST-15]
impl Caster {
    pub fn new(sample_rate: u32) -> Self {
        let (status_tx, status_rx) = unbounded();
        Self {
            sample_rate,
            outputs: Vec::new(),
            status_tx,
            status_rx,
            metadata: String::new(),
        }
    }

    pub fn is_active(&self) -> bool {
        !self.outputs.is_empty()
    }

    pub fn start(&mut self, cfg: OutputConfig, password: String) {
        self.stop(&cfg.id);
        let (pcm_tx, pcm_rx) = bounded(PCM_QUEUE);
        let (meta_tx, meta_rx) = unbounded();
        let stop = Arc::new(AtomicBool::new(false));
        let id = cfg.id.clone();
        let worker = Worker {
            cfg,
            password,
            sample_rate: self.sample_rate,
            pcm: pcm_rx,
            meta: meta_rx,
            stop: stop.clone(),
            status: self.status_tx.clone(),
            song: self.metadata.clone(),
        };
        let thread = thread::Builder::new()
            .name(format!("blast-cast-{id}"))
            .spawn(move || worker.run())
            .ok();
        self.outputs.push(Handle {
            id,
            pcm: pcm_tx,
            meta: meta_tx,
            stop,
            thread,
        });
    }

    pub fn stop(&mut self, id: &str) {
        if let Some(pos) = self.outputs.iter().position(|h| h.id == id) {
            let h = self.outputs.remove(pos);
            h.stop.store(true, Ordering::Relaxed);
            drop(h.pcm);
            drop(h.thread);
        }
    }

    pub fn stop_all(&mut self) {
        let ids: Vec<String> = self.outputs.iter().map(|h| h.id.clone()).collect();
        for id in ids {
            self.stop(&id);
        }
    }

    pub fn push(&self, chunk: Arc<[f32]>) {
        for h in &self.outputs {
            let _ = h.pcm.try_send(chunk.clone());
        }
    }

    pub fn set_metadata(&mut self, song: &str) {
        if song == self.metadata {
            return;
        }
        self.metadata = song.to_string();
        for h in &self.outputs {
            let _ = h.meta.send(self.metadata.clone());
        }
    }

    pub fn poll(&self) -> Vec<StatusEvent> {
        self.status_rx.try_iter().collect()
    }
}

impl Drop for Caster {
    fn drop(&mut self) {
        self.stop_all();
    }
}

// [CAST-16]
struct Worker {
    cfg: OutputConfig,
    password: String,
    sample_rate: u32,
    pcm: Receiver<Arc<[f32]>>,
    meta: Receiver<String>,
    stop: Arc<AtomicBool>,
    status: Sender<StatusEvent>,
    song: String,
}

// [CAST-17]
enum Outcome {
    Stopped,
    Lost(String),
    Fatal(String),
}

// [CAST-18]
impl Worker {
    fn report(&self, status: OutputStatus) {
        let _ = self.status.send(StatusEvent {
            id: self.cfg.id.clone(),
            status,
        });
    }

    fn stopped(&self) -> bool {
        self.stop.load(Ordering::Relaxed)
    }

    fn send_metadata(&self) {
        if self.song.is_empty() {
            return;
        }
        let cfg = self.cfg.clone();
        let pw = self.password.clone();
        let song = self.song.clone();
        thread::spawn(move || {
            let _ = update_metadata(&cfg, &pw, &song);
        });
    }

    fn run(mut self) {
        loop {
            if self.stopped() {
                break;
            }
            self.report(OutputStatus::Connecting);
            let outcome = match open_source(&self.cfg, &self.password, self.sample_rate) {
                Ok(sock) => self.stream(sock),
                Err(e) if e.is_fatal() => Outcome::Fatal(e.to_string()),
                Err(e) => Outcome::Lost(e.to_string()),
            };
            match outcome {
                Outcome::Stopped => break,
                Outcome::Fatal(msg) => {
                    self.report(OutputStatus::Error(msg));
                    return;
                }
                Outcome::Lost(msg) => {
                    self.report(OutputStatus::Retrying(msg));
                    let mut waited = Duration::ZERO;
                    while waited < RETRY_DELAY && !self.stopped() {
                        thread::sleep(Duration::from_millis(100));
                        waited += Duration::from_millis(100);
                        while self.pcm.try_recv().is_ok() {}
                    }
                }
            }
        }
        self.report(OutputStatus::Off);
    }

    fn stream(&mut self, mut sock: std::net::TcpStream) -> Outcome {
        let mut enc = match Mp3Encoder::new(self.sample_rate, self.cfg.bitrate, self.cfg.mono) {
            Ok(e) => e,
            Err(e) => return Outcome::Fatal(e),
        };
        while self.pcm.try_recv().is_ok() {}
        self.report(OutputStatus::Live);
        self.send_metadata();
        loop {
            if self.stopped() {
                let _ = sock.shutdown(std::net::Shutdown::Both);
                return Outcome::Stopped;
            }
            let mut new_song = None;
            while let Ok(s) = self.meta.try_recv() {
                new_song = Some(s);
            }
            if let Some(s) = new_song {
                self.song = s;
                self.send_metadata();
            }
            match self.pcm.recv_timeout(Duration::from_millis(200)) {
                Ok(chunk) => {
                    let bytes = match enc.encode(&chunk) {
                        Ok(b) => b,
                        Err(e) => return Outcome::Fatal(e),
                    };
                    if !bytes.is_empty() {
                        if let Err(e) = sock.write_all(bytes) {
                            return Outcome::Lost(format!("connexion interrompue ({e})"));
                        }
                    }
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return Outcome::Stopped,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    use std::net::TcpListener;

    #[test]
    fn streams_mp3_to_server_and_reconnects() {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        let server = thread::spawn(move || {
            let mut sizes = Vec::new();
            for round in 0..2 {
                let (mut s, _) = l.accept().unwrap();
                let mut head = Vec::new();
                let mut b = [0u8; 1];
                while !head.ends_with(b"\r\n\r\n") {
                    s.read_exact(&mut b).unwrap();
                    head.push(b[0]);
                }
                s.write_all(b"HTTP/1.1 200 OK\r\n\r\n").unwrap();
                let mut body = vec![0u8; 20_000];
                let mut got = 0;
                while got < body.len() {
                    match s.read(&mut body[got..]) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => got += n,
                    }
                }
                sizes.push(got);
                if round == 0 {
                    drop(s);
                }
            }
            sizes
        });

        let mut caster = Caster::new(44_100);
        caster.start(
            OutputConfig {
                id: "t".into(),
                host: "127.0.0.1".into(),
                port,
                ..Default::default()
            },
            "pw".into(),
        );
        let chunk: Arc<[f32]> = (0..2048)
            .map(|i| ((i / 2) as f32 * 0.06).sin() * 0.3)
            .collect::<Vec<_>>()
            .into();
        let mut seen = Vec::new();
        let deadline = std::time::Instant::now() + Duration::from_secs(30);
        while !server.is_finished() && std::time::Instant::now() < deadline {
            caster.push(chunk.clone());
            thread::sleep(Duration::from_millis(5));
            seen.extend(caster.poll().into_iter().map(|e| e.status));
        }
        assert!(
            server.is_finished(),
            "le serveur de test n'a pas reçu les deux connexions"
        );
        let sizes = server.join().unwrap();
        assert_eq!(sizes, vec![20_000, 20_000]);
        assert!(seen.contains(&OutputStatus::Live));
        assert!(seen.iter().any(|s| matches!(s, OutputStatus::Retrying(_))));
        caster.stop_all();
    }
}
