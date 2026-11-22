#![allow(clippy::result_large_err)]

use std::collections::{HashMap, HashSet};
use std::io::ErrorKind;
use std::net::{TcpStream, ToSocketAddrs};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use crossbeam_channel::{unbounded, Receiver, Sender};
use serde::Serialize;
use serde_json::{json, Value};
use tungstenite::{Message, WebSocket};

use crate::{auth_response, AuraError, AuraSettings};

// [AURA-08]
const EVENTS_ALL: u32 = 2047;
// [AURA-09]
const AUDIO_KINDS: &[&str] = &[
    "pulse_input_capture",
    "pulse_output_capture",
    "pipewire-audio-capture-input",
    "pipewire-audio-capture-output",
    "alsa_input_capture",
    "jack_output_capture",
    "wasapi_input_capture",
    "wasapi_output_capture",
    "wasapi_process_output_capture",
    "coreaudio_input_capture",
    "coreaudio_output_capture",
    "sck_audio_capture",
];

// [AURA-10]
#[derive(Debug, Clone, PartialEq)]
pub enum ObsCommand {
    SetScene(String),
    ToggleStream,
    ToggleRecord,
    ToggleRecordPause,
    SaveReplay,
    Screenshot(PathBuf),
    SetVolume { input: String, mul: f32 },
    ToggleMute(String),
    SetText { source: String, text: String },
    Refresh,
}

// [AURA-11]
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ObsInput {
    pub name: String,
    pub kind: String,
}

// [AURA-12]
#[derive(Debug, Clone, PartialEq)]
pub enum ObsEvent {
    Connecting,
    Connected {
        obs_version: String,
        websocket_version: String,
    },
    Disconnected {
        reason: String,
        retrying: bool,
    },
    Scenes {
        scenes: Vec<String>,
        current: Option<String>,
    },
    CurrentScene(String),
    Stream {
        active: bool,
        duration_ms: Option<u64>,
    },
    Record {
        active: bool,
        paused: bool,
        duration_ms: Option<u64>,
    },
    Inputs(Vec<ObsInput>),
    InputMuted {
        name: String,
        muted: bool,
    },
    InputVolume {
        name: String,
        mul: f32,
    },
    Info(String),
    Error(String),
}

// [AURA-13]
pub struct AuraClient {
    commands: Sender<ObsCommand>,
    events: Receiver<ObsEvent>,
    stop: Arc<AtomicBool>,
}

impl AuraClient {
    pub fn start(settings: AuraSettings, password: Option<String>) -> Self {
        let (cmd_tx, cmd_rx) = unbounded();
        let (evt_tx, evt_rx) = unbounded();
        let stop = Arc::new(AtomicBool::new(false));
        let s = stop.clone();
        let _ = thread::Builder::new()
            .name("blast-aura".into())
            .spawn(move || run(settings, password, cmd_rx, evt_tx, s));
        Self {
            commands: cmd_tx,
            events: evt_rx,
            stop,
        }
    }

    pub fn send(&self, cmd: ObsCommand) {
        let _ = self.commands.send(cmd);
    }

    pub fn poll(&self) -> Vec<ObsEvent> {
        self.events.try_iter().collect()
    }
}

impl Drop for AuraClient {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

// [AURA-14]
enum End {
    Stopped,
    Lost(String),
    Fatal(String),
}

// [AURA-15]
fn run(
    settings: AuraSettings,
    password: Option<String>,
    commands: Receiver<ObsCommand>,
    events: Sender<ObsEvent>,
    stop: Arc<AtomicBool>,
) {
    let mut delay = Duration::from_secs(2);
    while !stop.load(Ordering::Relaxed) {
        let _ = events.send(ObsEvent::Connecting);
        let started = Instant::now();
        match session(&settings, password.as_deref(), &commands, &events, &stop) {
            End::Stopped => break,
            End::Fatal(reason) => {
                let _ = events.send(ObsEvent::Disconnected {
                    reason,
                    retrying: false,
                });
                return;
            }
            End::Lost(reason) => {
                if started.elapsed() > Duration::from_secs(30) {
                    delay = Duration::from_secs(2);
                }
                let _ = events.send(ObsEvent::Disconnected {
                    reason,
                    retrying: true,
                });
                let until = Instant::now() + delay;
                while Instant::now() < until && !stop.load(Ordering::Relaxed) {
                    thread::sleep(Duration::from_millis(100));
                    while commands.try_recv().is_ok() {}
                }
                delay = (delay * 2).min(Duration::from_secs(30));
            }
        }
    }
    let _ = events.send(ObsEvent::Disconnected {
        reason: "déconnecté".into(),
        retrying: false,
    });
}

// [AURA-16]
fn would_block(e: &tungstenite::Error) -> bool {
    matches!(e, tungstenite::Error::Io(io) if matches!(io.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut))
}

// [AURA-17]
struct Session<'a> {
    ws: WebSocket<TcpStream>,
    events: &'a Sender<ObsEvent>,
    seq: u64,
    current_scene: Option<String>,
    audio_inputs: HashSet<String>,
    pending_input: HashMap<String, String>,
}

// [AURA-18]
impl Session<'_> {
    fn request(
        &mut self,
        kind: &str,
        id_prefix: &str,
        data: Value,
    ) -> Result<(), tungstenite::Error> {
        self.seq += 1;
        let id = format!("{id_prefix}#{}", self.seq);
        if let Some(name) = data["inputName"].as_str() {
            if matches!(id_prefix, "mute" | "vol") {
                self.pending_input.insert(id.clone(), name.to_string());
            }
        }
        let mut d = json!({"requestType": kind, "requestId": id});
        if !data.is_null() {
            d["requestData"] = data;
        }
        self.ws
            .send(Message::text(json!({"op": 6, "d": d}).to_string()))
    }

    fn emit(&self, e: ObsEvent) {
        let _ = self.events.send(e);
    }

    fn refresh(&mut self) -> Result<(), tungstenite::Error> {
        self.request("GetSceneList", "scenes", Value::Null)?;
        self.request("GetStreamStatus", "stream", Value::Null)?;
        self.request("GetRecordStatus", "record", Value::Null)?;
        self.request("GetInputList", "inputs", Value::Null)
    }

    fn command(&mut self, cmd: ObsCommand) -> Result<(), tungstenite::Error> {
        match cmd {
            ObsCommand::SetScene(name) => {
                self.request("SetCurrentProgramScene", "act", json!({"sceneName": name}))
            }
            ObsCommand::ToggleStream => self.request("ToggleStream", "act", Value::Null),
            ObsCommand::ToggleRecord => self.request("ToggleRecord", "act", Value::Null),
            ObsCommand::ToggleRecordPause => self.request("ToggleRecordPause", "act", Value::Null),
            ObsCommand::SaveReplay => self.request("SaveReplayBuffer", "replay", Value::Null),
            ObsCommand::Screenshot(path) => match self.current_scene.clone() {
                Some(scene) => self.request(
                    "SaveSourceScreenshot",
                    "snap",
                    json!({"sourceName": scene, "imageFormat": "png",
                           "imageFilePath": path.to_string_lossy()}),
                ),
                None => {
                    self.emit(ObsEvent::Error("aucune scène active dans OBS".into()));
                    Ok(())
                }
            },
            ObsCommand::SetVolume { input, mul } => self.request(
                "SetInputVolume",
                "act",
                json!({"inputName": input, "inputVolumeMul": mul.clamp(0.0, 1.0)}),
            ),
            ObsCommand::ToggleMute(input) => {
                self.request("ToggleInputMute", "act", json!({"inputName": input}))
            }
            ObsCommand::SetText { source, text } => self.request(
                "SetInputSettings",
                "text",
                json!({"inputName": source, "inputSettings": {"text": text}}),
            ),
            ObsCommand::Refresh => self.refresh(),
        }
    }

    fn on_event(&mut self, d: &Value) -> Result<(), tungstenite::Error> {
        let data = &d["eventData"];
        match d["eventType"].as_str().unwrap_or("") {
            "CurrentProgramSceneChanged" => {
                if let Some(n) = data["sceneName"].as_str() {
                    self.current_scene = Some(n.to_string());
                    self.emit(ObsEvent::CurrentScene(n.to_string()));
                }
            }
            "SceneListChanged" | "SceneCreated" | "SceneRemoved" | "SceneNameChanged" => {
                self.request("GetSceneList", "scenes", Value::Null)?;
            }
            "StreamStateChanged" => {
                let active = data["outputActive"].as_bool().unwrap_or(false);
                self.emit(ObsEvent::Stream {
                    active,
                    duration_ms: active.then_some(0),
                });
            }
            "RecordStateChanged" => {
                let state = data["outputState"].as_str().unwrap_or("");
                let active = data["outputActive"].as_bool().unwrap_or(false);
                let paused = state == "OBS_WEBSOCKET_OUTPUT_PAUSED";
                let fresh = state == "OBS_WEBSOCKET_OUTPUT_STARTED";
                self.emit(ObsEvent::Record {
                    active,
                    paused,
                    duration_ms: fresh.then_some(0),
                });
            }
            "InputMuteStateChanged" => {
                if let (Some(n), Some(m)) =
                    (data["inputName"].as_str(), data["inputMuted"].as_bool())
                {
                    self.emit(ObsEvent::InputMuted {
                        name: n.to_string(),
                        muted: m,
                    });
                }
            }
            "InputVolumeChanged" => {
                if let (Some(n), Some(v)) =
                    (data["inputName"].as_str(), data["inputVolumeMul"].as_f64())
                {
                    self.emit(ObsEvent::InputVolume {
                        name: n.to_string(),
                        mul: v as f32,
                    });
                }
            }
            "InputCreated" | "InputRemoved" | "InputNameChanged" => {
                self.request("GetInputList", "inputs", Value::Null)?;
            }
            "ReplayBufferSaved" => {
                let path = data["savedReplayPath"].as_str().unwrap_or("");
                self.emit(ObsEvent::Info(format!("Replay enregistré : {path}")));
            }
            _ => {}
        }
        Ok(())
    }

    fn on_response(&mut self, d: &Value) -> Result<(), tungstenite::Error> {
        let id = d["requestId"].as_str().unwrap_or("");
        let prefix = id.split('#').next().unwrap_or("");
        let ok = d["requestStatus"]["result"].as_bool().unwrap_or(false);
        let rd = &d["responseData"];
        let input_name = self.pending_input.remove(id);
        if !ok {
            if prefix == "mute" || prefix == "vol" {
                return Ok(());
            }
            let comment = d["requestStatus"]["comment"]
                .as_str()
                .unwrap_or("requête refusée");
            let what = match prefix {
                "replay" => "Replay",
                "snap" => "Capture",
                "text" => "Texte Now Playing",
                _ => d["requestType"].as_str().unwrap_or("OBS"),
            };
            self.emit(ObsEvent::Error(format!("{what} : {comment}")));
            return Ok(());
        }
        match prefix {
            "scenes" => {
                let mut scenes: Vec<String> = rd["scenes"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(|s| s["sceneName"].as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();
                scenes.reverse();
                let current = rd["currentProgramSceneName"].as_str().map(String::from);
                self.current_scene = current.clone();
                self.emit(ObsEvent::Scenes { scenes, current });
            }
            "stream" => self.emit(ObsEvent::Stream {
                active: rd["outputActive"].as_bool().unwrap_or(false),
                duration_ms: rd["outputDuration"].as_u64(),
            }),
            "record" => self.emit(ObsEvent::Record {
                active: rd["outputActive"].as_bool().unwrap_or(false),
                paused: rd["outputPaused"].as_bool().unwrap_or(false),
                duration_ms: rd["outputDuration"].as_u64(),
            }),
            "inputs" => {
                let inputs: Vec<ObsInput> = rd["inputs"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(|i| {
                                let kind = i["unversionedInputKind"]
                                    .as_str()
                                    .or(i["inputKind"].as_str())?;
                                let name = i["inputName"].as_str()?;
                                AUDIO_KINDS.contains(&kind).then(|| ObsInput {
                                    name: name.to_string(),
                                    kind: kind.to_string(),
                                })
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                self.audio_inputs = inputs.iter().map(|i| i.name.clone()).collect();
                for i in &inputs {
                    self.request("GetInputMute", "mute", json!({"inputName": i.name}))?;
                    self.request("GetInputVolume", "vol", json!({"inputName": i.name}))?;
                }
                self.emit(ObsEvent::Inputs(inputs));
            }
            "mute" => {
                if let (Some(muted), Some(name)) = (rd["inputMuted"].as_bool(), input_name) {
                    self.emit(ObsEvent::InputMuted { name, muted });
                }
            }
            "vol" => {
                if let (Some(mul), Some(name)) = (rd["inputVolumeMul"].as_f64(), input_name) {
                    self.emit(ObsEvent::InputVolume {
                        name,
                        mul: mul as f32,
                    });
                }
            }
            "snap" => self.emit(ObsEvent::Info("Capture d'écran enregistrée".into())),
            _ => {}
        }
        Ok(())
    }
}

// [AURA-19]
fn handshake(
    settings: &AuraSettings,
    password: Option<&str>,
) -> Result<(WebSocket<TcpStream>, String, String), AuraError> {
    let addr = (settings.host.trim(), settings.port)
        .to_socket_addrs()
        .map_err(|e| AuraError::Connect(e.to_string()))?
        .next()
        .ok_or_else(|| AuraError::Connect("adresse inconnue".into()))?;
    let stream = TcpStream::connect_timeout(&addr, Duration::from_secs(4))
        .map_err(|e| AuraError::Connect(e.to_string()))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|e| AuraError::Connect(e.to_string()))?;
    let (mut ws, _) = tungstenite::client(settings.url(), stream)
        .map_err(|e| AuraError::Connect(e.to_string()))?;
    let hello = crate::read_json(&mut ws)?;
    let websocket_version = hello["d"]["obsWebSocketVersion"]
        .as_str()
        .unwrap_or("?")
        .to_string();
    let mut identify = json!({"op": 1, "d": {"rpcVersion": 1, "eventSubscriptions": EVENTS_ALL}});
    if let Some(auth) = hello["d"].get("authentication") {
        let pw = password
            .filter(|p| !p.is_empty())
            .ok_or(AuraError::PasswordRequired)?;
        identify["d"]["authentication"] = Value::String(auth_response(
            pw,
            auth["salt"].as_str().unwrap_or_default(),
            auth["challenge"].as_str().unwrap_or_default(),
        ));
    }
    ws.send(Message::text(identify.to_string()))
        .map_err(|e| AuraError::Protocol(e.to_string()))?;
    let identified = crate::read_json(&mut ws)?;
    if identified["op"] != 2 {
        return Err(AuraError::Protocol("identification refusée".into()));
    }
    ws.send(Message::text(
        json!({"op": 6, "d": {"requestType": "GetVersion", "requestId": "version"}}).to_string(),
    ))
    .map_err(|e| AuraError::Protocol(e.to_string()))?;
    let mut obs_version = "?".to_string();
    for _ in 0..8 {
        let msg = crate::read_json(&mut ws)?;
        if msg["op"] == 7 && msg["d"]["requestId"] == "version" {
            obs_version = msg["d"]["responseData"]["obsVersion"]
                .as_str()
                .unwrap_or("?")
                .to_string();
            break;
        }
    }
    ws.get_ref()
        .set_read_timeout(Some(Duration::from_millis(50)))
        .map_err(|e| AuraError::Connect(e.to_string()))?;
    Ok((ws, obs_version, websocket_version))
}

// [AURA-20]
fn session(
    settings: &AuraSettings,
    password: Option<&str>,
    commands: &Receiver<ObsCommand>,
    events: &Sender<ObsEvent>,
    stop: &AtomicBool,
) -> End {
    let (ws, obs_version, websocket_version) = match handshake(settings, password) {
        Ok(v) => v,
        Err(e @ (AuraError::AuthFailed | AuraError::PasswordRequired)) => {
            return End::Fatal(e.to_string())
        }
        Err(e) => return End::Lost(e.to_string()),
    };
    let _ = events.send(ObsEvent::Connected {
        obs_version,
        websocket_version,
    });
    let mut s = Session {
        ws,
        events,
        seq: 0,
        current_scene: None,
        audio_inputs: HashSet::new(),
        pending_input: HashMap::new(),
    };
    if let Err(e) = s.refresh() {
        return End::Lost(e.to_string());
    }
    loop {
        if stop.load(Ordering::Relaxed) {
            let _ = s.ws.close(None);
            let _ = s.ws.flush();
            return End::Stopped;
        }
        while let Ok(cmd) = commands.try_recv() {
            if let Err(e) = s.command(cmd) {
                return End::Lost(e.to_string());
            }
        }
        match s.ws.read() {
            Ok(Message::Text(t)) => {
                let Ok(msg) = serde_json::from_str::<Value>(t.as_str()) else {
                    continue;
                };
                let r = match msg["op"].as_u64() {
                    Some(5) => s.on_event(&msg["d"]),
                    Some(7) => s.on_response(&msg["d"]),
                    _ => Ok(()),
                };
                if let Err(e) = r {
                    return End::Lost(e.to_string());
                }
            }
            Ok(Message::Close(_)) => return End::Lost("OBS a fermé la connexion".into()),
            Ok(_) => {}
            Err(e) if would_block(&e) => {
                if let Err(e) = s.ws.flush() {
                    if !would_block(&e) {
                        return End::Lost(e.to_string());
                    }
                }
            }
            Err(e) => return End::Lost(e.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    fn text(ws: &mut WebSocket<TcpStream>) -> Value {
        loop {
            if let Message::Text(t) = ws.read().unwrap() {
                return serde_json::from_str(t.as_str()).unwrap();
            }
        }
    }

    fn respond(ws: &mut WebSocket<TcpStream>, req: &Value, data: Value) {
        ws.send(Message::text(
            json!({"op":7,"d":{"requestType":req["d"]["requestType"],"requestId":req["d"]["requestId"],
                "requestStatus":{"result":true,"code":100},"responseData":data}})
            .to_string(),
        ))
        .unwrap();
    }

    fn fake_obs() -> (u16, thread::JoinHandle<Vec<String>>) {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        let h = thread::spawn(move || {
            let (s, _) = l.accept().unwrap();
            let mut ws = tungstenite::accept(s).unwrap();
            ws.send(Message::text(
                json!({"op":0,"d":{"obsWebSocketVersion":"5.5.0","rpcVersion":1}}).to_string(),
            ))
            .unwrap();
            let id = text(&mut ws);
            assert_eq!(id["d"]["eventSubscriptions"], EVENTS_ALL);
            ws.send(Message::text(
                json!({"op":2,"d":{"negotiatedRpcVersion":1}}).to_string(),
            ))
            .unwrap();
            let mut seen = Vec::new();
            loop {
                let req = text(&mut ws);
                let kind = req["d"]["requestType"].as_str().unwrap().to_string();
                seen.push(kind.clone());
                match kind.as_str() {
                    "GetVersion" => respond(&mut ws, &req, json!({"obsVersion":"31.1.0"})),
                    "GetSceneList" => respond(
                        &mut ws,
                        &req,
                        json!({"currentProgramSceneName":"Plateau",
                        "scenes":[{"sceneName":"Pause"},{"sceneName":"Plateau"},{"sceneName":"Intro"}]}),
                    ),
                    "GetStreamStatus" => respond(
                        &mut ws,
                        &req,
                        json!({"outputActive":true,"outputDuration":65000}),
                    ),
                    "GetRecordStatus" => respond(
                        &mut ws,
                        &req,
                        json!({"outputActive":false,"outputPaused":false,"outputDuration":0}),
                    ),
                    "GetInputList" => respond(
                        &mut ws,
                        &req,
                        json!({"inputs":[
                        {"inputName":"Micro","inputKind":"pulse_input_capture","unversionedInputKind":"pulse_input_capture"},
                        {"inputName":"Webcam","inputKind":"v4l2_input","unversionedInputKind":"v4l2_input"}]}),
                    ),
                    "GetInputMute" => respond(&mut ws, &req, json!({"inputMuted":false})),
                    "GetInputVolume" => respond(
                        &mut ws,
                        &req,
                        json!({"inputVolumeMul":0.5,"inputVolumeDb":-6.0}),
                    ),
                    "SetCurrentProgramScene" => {
                        respond(&mut ws, &req, Value::Null);
                        let name = req["d"]["requestData"]["sceneName"].clone();
                        ws.send(Message::text(
                            json!({"op":5,"d":{"eventType":"CurrentProgramSceneChanged",
                            "eventIntent":4,"eventData":{"sceneName":name}}})
                            .to_string(),
                        ))
                        .unwrap();
                    }
                    "SaveReplayBuffer" => {
                        ws.send(Message::text(json!({"op":7,"d":{"requestType":"SaveReplayBuffer","requestId":req["d"]["requestId"],
                            "requestStatus":{"result":false,"code":604,"comment":"Replay buffer is not active."}}}).to_string())).unwrap();
                    }
                    "SetInputSettings" => {
                        respond(&mut ws, &req, Value::Null);
                        assert_eq!(
                            req["d"]["requestData"]["inputSettings"]["text"],
                            "D-Goth - Void Relay"
                        );
                        let _ = ws.close(None);
                        let _ = ws.flush();
                        return seen;
                    }
                    _ => respond(&mut ws, &req, Value::Null),
                }
            }
        });
        (port, h)
    }

    fn wait_for(
        c: &AuraClient,
        seen: &mut Vec<ObsEvent>,
        pred: impl Fn(&ObsEvent) -> bool,
    ) -> bool {
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut checked = seen.len();
        while Instant::now() < deadline {
            seen.extend(c.poll());
            if seen[checked..].iter().any(&pred) {
                return true;
            }
            checked = seen.len();
            thread::sleep(Duration::from_millis(10));
        }
        false
    }

    #[test]
    fn full_session_against_fake_obs() {
        let (port, server) = fake_obs();
        let c = AuraClient::start(
            AuraSettings {
                port,
                ..Default::default()
            },
            None,
        );
        let mut ev = Vec::new();
        assert!(wait_for(
            &c,
            &mut ev,
            |e| matches!(e, ObsEvent::Connected { obs_version, .. } if obs_version == "31.1.0")
        ));
        assert!(wait_for(
            &c,
            &mut ev,
            |e| matches!(e, ObsEvent::InputVolume { name, .. } if name == "Micro")
        ));
        assert!(ev.contains(&ObsEvent::Scenes {
            scenes: vec!["Intro".into(), "Plateau".into(), "Pause".into()],
            current: Some("Plateau".into())
        }));
        assert!(ev.contains(&ObsEvent::Stream {
            active: true,
            duration_ms: Some(65000)
        }));
        assert!(ev.contains(&ObsEvent::Inputs(vec![ObsInput {
            name: "Micro".into(),
            kind: "pulse_input_capture".into()
        }])));
        c.send(ObsCommand::SetScene("Intro".into()));
        assert!(wait_for(&c, &mut ev, |e| *e
            == ObsEvent::CurrentScene("Intro".into())));
        c.send(ObsCommand::SaveReplay);
        assert!(wait_for(
            &c,
            &mut ev,
            |e| matches!(e, ObsEvent::Error(m) if m.contains("Replay buffer is not active"))
        ));
        c.send(ObsCommand::SetText {
            source: "Titre".into(),
            text: "D-Goth - Void Relay".into(),
        });
        assert!(wait_for(&c, &mut ev, |e| matches!(
            e,
            ObsEvent::Disconnected { retrying: true, .. }
        )));
        let seen = server.join().unwrap();
        assert!(seen.contains(&"SetCurrentProgramScene".to_string()));
        drop(c);
    }

    #[test]
    fn unreachable_obs_retries() {
        let c = AuraClient::start(
            AuraSettings {
                port: 1,
                ..Default::default()
            },
            None,
        );
        let mut ev = Vec::new();
        assert!(wait_for(&c, &mut ev, |e| matches!(
            e,
            ObsEvent::Disconnected { retrying: true, .. }
        )));
    }
}
