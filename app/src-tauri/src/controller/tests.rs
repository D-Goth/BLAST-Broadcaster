use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use blast_core::{DeckId, DeckState, DeckStatus, Event};
use blast_library::{History, Kind, NewPlay, SqliteStore};
use crossbeam_channel::unbounded;
use serde_json::Value;

use super::views::*;
use super::*;
use crate::config::AppConfig;

type Log = Arc<Mutex<Vec<(&'static str, Value)>>>;

struct RecordingUi(Log);

impl Ui for RecordingUi {
    fn emit(&self, event: &'static str, payload: Value) {
        self.0.lock().unwrap().push((event, payload));
    }

    fn pictures_dir(&self) -> PathBuf {
        std::env::temp_dir()
    }
}

struct Harness {
    c: Controller,
    log: Log,
    dir: PathBuf,
}

impl Harness {
    fn new(name: &str) -> Self {
        let log: Log = Arc::default();
        let (_tx, rx) = unbounded();
        let library = SqliteStore::open_in_memory()
            .and_then(|s| History::new(Box::new(s), 200))
            .ok();
        let c = Controller::new(
            Box::new(RecordingUi(log.clone())),
            rx,
            Arc::new(Secrets::default()),
            AppConfig::default(),
            None,
            Audio::none(44_100),
            library,
        );
        let dir = std::env::temp_dir().join(format!("blast-ctl-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Self { c, log, dir }
    }

    fn wav(&self, name: &str) -> PathBuf {
        let path = self.dir.join(name);
        write_wav(&path, 44_100, 22_050);
        path
    }

    fn work(&mut self, n: usize) {
        for _ in 0..n {
            let w = self
                .c
                .work_rx
                .recv_timeout(Duration::from_secs(10))
                .expect("travail attendu");
            self.c.on_work(w);
        }
    }

    fn last(&self, event: &str) -> Value {
        self.log
            .lock()
            .unwrap()
            .iter()
            .rev()
            .find(|(e, _)| *e == event)
            .map(|(_, v)| v.clone())
            .unwrap_or(Value::Null)
    }

    fn notices(&self, level: &str) -> Vec<String> {
        self.log
            .lock()
            .unwrap()
            .iter()
            .filter(|(e, v)| *e == EV_NOTICE && v["level"] == level)
            .map(|(_, v)| v["message"].as_str().unwrap_or_default().to_string())
            .collect()
    }

    fn tick(&mut self, a: DeckState, b: DeckState) {
        let frames = |i: usize| self.c.decks[i].status.frames;
        let decks = [
            DeckStatus {
                state: a,
                position: 100,
                frames: frames(0),
            },
            DeckStatus {
                state: b,
                position: 0,
                frames: frames(1),
            },
        ];
        self.c.on_event(Event::Tick {
            decks,
            crossfader: self.c.crossfader,
            peak: [0.0; 2],
            sfx: 0,
            mic_peak: 0.0,
            mic_open: false,
            duck: 1.0,
            reduction: 0.0,
        });
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn write_wav(path: &Path, rate: u32, frames: u32) {
    let data_len = frames * 4;
    let mut b = Vec::with_capacity(44 + data_len as usize);
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + data_len).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&2u16.to_le_bytes());
    b.extend_from_slice(&rate.to_le_bytes());
    b.extend_from_slice(&(rate * 4).to_le_bytes());
    b.extend_from_slice(&4u16.to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&data_len.to_le_bytes());
    for i in 0..frames {
        let s = ((i as f32 * 0.05).sin() * 8000.0) as i16;
        b.extend_from_slice(&s.to_le_bytes());
        b.extend_from_slice(&s.to_le_bytes());
    }
    std::fs::write(path, b).unwrap();
}

fn entry(path: &str, title: &str, artist: &str) -> Entry {
    Entry {
        path: PathBuf::from(path),
        title: title.into(),
        artist: artist.into(),
        ..Default::default()
    }
}

#[test]
fn ui_commands_parse_from_json() {
    let cmd: UiCommand =
        serde_json::from_str(r#"{"type":"load_entry","id":3,"deck":"B"}"#).unwrap();
    assert!(matches!(
        cmd,
        UiCommand::LoadEntry {
            id: 3,
            deck: Deck::B
        }
    ));
    let cmd: UiCommand =
        serde_json::from_str(r#"{"type":"obs","action":"scene","name":"Live"}"#).unwrap();
    assert!(matches!(cmd, UiCommand::Obs(ObsAction::Scene { .. })));
    assert!(serde_json::from_str::<UiCommand>(r#"{"type":"config"}"#).is_err());
}

#[test]
fn adding_a_folder_fills_the_playlist() {
    let mut h = Harness::new("add");
    h.wav("01.wav");
    h.wav("02.wav");
    std::fs::write(h.dir.join("notes.txt"), "x").unwrap();
    h.c.handle(UiCommand::AddPaths {
        paths: vec![h.dir.clone()],
        index: None,
    });
    h.work(1);
    let pl = h.last(EV_PLAYLIST);
    assert_eq!(pl["entries"].as_array().map(Vec::len), Some(2));
    assert!(pl["total"].as_f64().unwrap() > 0.9);
}

#[test]
fn files_dropped_on_a_deck_are_loaded() {
    let mut h = Harness::new("deck");
    let a = h.wav("a.wav");
    let b = h.wav("b.wav");
    h.c.handle(UiCommand::LoadPaths {
        deck: Deck::A,
        paths: vec![a, b],
    });
    h.work(2);
    let st = h.last(EV_STATE);
    assert_eq!(st["decks"][0]["state"], "loaded");
    assert!((st["decks"][0]["duration"].as_f64().unwrap() - 0.5).abs() < 0.01);
    assert_eq!(
        h.last(EV_PLAYLIST)["entries"].as_array().map(Vec::len),
        Some(1)
    );
}

fn old_play(at: i64) -> NewPlay {
    NewPlay {
        path: "/x/old.mp3".into(),
        title: "Ancien".into(),
        artist: "Artiste X".into(),
        kind: Kind::Morceau,
        started_at: at,
        on_air: true,
        listeners: None,
    }
}

#[test]
fn separation_skips_an_artist_played_recently() {
    let mut h = Harness::new("rules");
    let now = now_secs();
    h.c.record_play(old_play(now - 60));
    h.c.playlist.insert(
        vec![
            entry("/x/1.mp3", "Encore X", "Artiste X"),
            entry("/y/1.mp3", "Autre", "Artiste Y"),
        ],
        None,
    );
    h.c.emit_playlist();
    let pl = h.last(EV_PLAYLIST);
    assert!(pl["entries"][0]["blocked"].is_string());
    assert!(pl["entries"][1]["blocked"].is_null());
    let next = h.c.take_next_auto().unwrap();
    assert_eq!(next.title, "Autre");
    assert!(h.notices("warn").is_empty());
}

#[test]
fn separation_falls_back_with_a_warning() {
    let mut h = Harness::new("fallback");
    h.c.record_play(old_play(now_secs() - 60));
    h.c.playlist
        .insert(vec![entry("/x/1.mp3", "Encore X", "Artiste X")], None);
    assert_eq!(h.c.take_next_auto().unwrap().title, "Encore X");
    assert_eq!(h.notices("warn").len(), 1);
}

#[test]
fn next_with_an_empty_playlist_warns() {
    let mut h = Harness::new("next");
    h.c.handle(UiCommand::Next { deck: Deck::A });
    assert_eq!(h.notices("warn"), ["Plus aucun titre dans la playlist."]);
}

#[test]
fn unreadable_entries_are_refused_on_decks() {
    let mut h = Harness::new("bad");
    std::fs::write(h.dir.join("cassé.mp3"), b"pas de l'audio").unwrap();
    h.c.handle(UiCommand::AddPaths {
        paths: vec![h.dir.clone()],
        index: None,
    });
    h.work(1);
    let id = h.c.playlist.entries()[0].id;
    h.c.handle(UiCommand::LoadEntry { id, deck: Deck::A });
    assert_eq!(h.notices("error").len(), 1);
    assert_eq!(h.c.playlist.entries().len(), 1);
}

#[test]
fn plays_are_written_to_the_history() {
    let mut h = Harness::new("history");
    let a = h.wav("a.wav");
    h.c.handle(UiCommand::LoadPaths {
        deck: Deck::A,
        paths: vec![a],
    });
    h.work(2);
    h.tick(DeckState::Playing, DeckState::Empty);
    let hist = h.last(EV_HISTORY);
    assert_eq!(hist["all"].as_array().map(Vec::len), Some(1));
    assert_eq!(hist["morceau"].as_array().map(Vec::len), Some(1));
    assert_eq!(hist["jingle"].as_array().map(Vec::len), Some(0));
    assert!(hist["current"].is_i64());
    h.c.on_event(Event::Ended(DeckId::A));
    assert!(h.last(EV_HISTORY)["current"].is_null());
    let history = h.c.auto.history.as_ref().unwrap();
    assert_eq!(history.rows(None).len(), 1);
    assert!(history.rows(None)[0].duration_played.is_some());
    assert_eq!(history.store_lookups(), 0);
}

#[test]
fn automix_cues_both_decks() {
    let mut h = Harness::new("automix");
    h.wav("1.wav");
    h.wav("2.wav");
    h.wav("3.wav");
    h.c.handle(UiCommand::AutoMix { enabled: true });
    h.c.handle(UiCommand::AddPaths {
        paths: vec![h.dir.clone()],
        index: None,
    });
    h.work(3);
    assert!(h.c.decks[0].is_cued());
    assert!(h.c.decks[1].is_cued());
    assert_eq!(h.c.playlist.entries().len(), 1);
}

#[test]
fn casting_without_outputs_warns() {
    let mut h = Harness::new("cast");
    h.c.handle(UiCommand::CastStart);
    assert_eq!(h.notices("warn").len(), 1);
    assert_eq!(h.last(EV_CAST), Value::Null);
}

#[test]
fn jingles_and_pubs_are_typed_and_exempt_from_rules() {
    let mut h = Harness::new("kinds");
    h.c.record_play(old_play(now_secs() - 60));
    std::fs::create_dir_all(h.dir.join("Jingles")).unwrap();
    std::fs::create_dir_all(h.dir.join("Pubs")).unwrap();
    write_wav(&h.dir.join("Jingles/id.wav"), 44_100, 4_410);
    write_wav(&h.dir.join("Pubs/garage.wav"), 44_100, 4_410);
    h.c.handle(UiCommand::AddPaths {
        paths: vec![h.dir.clone()],
        index: None,
    });
    h.work(1);
    let kinds: Vec<Kind> = h.c.playlist.entries().iter().map(|e| e.kind).collect();
    assert_eq!(kinds, [Kind::Jingle, Kind::Pub]);
    let mut e = h.c.playlist.entries()[0].clone();
    e.artist = "Artiste X".into();
    assert!(h.c.violation_for(&e, now_secs()).is_none());
    let mut cfg = AppConfig::default();
    cfg.categories.jingle_keywords.clear();
    h.c.handle(UiCommand::Config(Box::new(cfg)));
    assert_eq!(h.c.playlist.entries()[0].kind, Kind::Morceau);
}

#[test]
fn jingle_box_plays_and_logs_jingles() {
    let mut h = Harness::new("jbox");
    let dir = h.dir.join("Jingles");
    std::fs::create_dir_all(&dir).unwrap();
    write_wav(&dir.join("a.wav"), 44_100, 4_410);
    write_wav(&dir.join("b.wav"), 44_100, 4_410);
    let mut cfg = AppConfig::default();
    cfg.jingles.dirs = vec![dir];
    h.c.handle(UiCommand::Config(Box::new(cfg)));
    h.work(3);
    let view = h.last(EV_JINGLES);
    assert_eq!(view["items"].as_array().map(Vec::len), Some(2));
    assert_eq!(view["items"][1]["ready"], true);
    h.c.handle(UiCommand::JinglePlay { index: 1 });
    assert_eq!(h.last(EV_JINGLES)["items"][1]["playing"], true);
    let hist = h.last(EV_HISTORY);
    assert_eq!(hist["jingle"].as_array().map(Vec::len), Some(1));
    assert_eq!(hist["jingle"][0]["kind"], "jingle");
    assert_eq!(hist["morceau"].as_array().map(Vec::len), Some(0));
    h.c.on_event(Event::SfxEnded(blast_core::CART_BASE + 1));
    assert_eq!(h.last(EV_JINGLES)["items"][1]["playing"], false);
    assert!(h.last(EV_HISTORY)["jingle"][0]["duration_played"].is_f64());
}

#[test]
fn clockwheel_fills_the_playlist_from_the_library() {
    let mut h = Harness::new("clock");
    let rock = h.dir.join("Rock");
    let jingles = h.dir.join("Jingles");
    std::fs::create_dir_all(&rock).unwrap();
    std::fs::create_dir_all(&jingles).unwrap();
    for i in 0..4 {
        write_wav(&rock.join(format!("r{i}.wav")), 44_100, 22_050);
    }
    write_wav(&jingles.join("id.wav"), 44_100, 4_410);
    let mut cfg = AppConfig::default();
    cfg.library.dirs = vec![h.dir.clone()];
    cfg.rotation = blast_library::Rotation {
        categories: vec![
            blast_library::RotationCategory {
                id: "rock".into(),
                name: "Rock".into(),
                color: "#ff0000".into(),
                rule: blast_library::CategoryRule {
                    folders: vec![rock.to_string_lossy().into_owned()],
                    ..Default::default()
                },
            },
            blast_library::RotationCategory {
                id: "ident".into(),
                name: "Habillage".into(),
                color: "#00ff00".into(),
                rule: blast_library::CategoryRule {
                    kinds: vec![Kind::Jingle],
                    ..Default::default()
                },
            },
        ],
        clocks: vec![blast_library::Clock {
            id: "jour".into(),
            name: "Journée".into(),
            color: "#0000ff".into(),
            slots: vec!["ident".into(), "rock".into(), "rock".into()],
        }],
        programs: Vec::new(),
        schedule: Vec::new(),
        default_clock: Some("jour".into()),
    };
    h.c.handle(UiCommand::Config(Box::new(cfg)));
    loop {
        let w =
            h.c.work_rx
                .recv_timeout(Duration::from_secs(10))
                .expect("scan");
        let done = matches!(w, Work::LibraryDone { .. });
        h.c.on_work(w);
        if done {
            break;
        }
    }
    assert_eq!(h.last(EV_LIBRARY)["tracks"], 5);
    assert_eq!(h.last(EV_STATE)["clock"]["name"], "Journée");
    h.c.handle(UiCommand::AutoMix { enabled: true });
    let loaded: Vec<Kind> = h.c.decks.iter().map(|d| d.meta.kind).collect();
    assert_eq!(loaded, [Kind::Jingle, Kind::Morceau]);
    h.c.fill_automix();
    assert_eq!(h.c.playlist.entries().len(), 3);
    assert!(h
        .c
        .playlist
        .entries()
        .iter()
        .all(|e| e.kind == Kind::Morceau));
    assert_eq!(h.notices("warn").len(), 1);
    let paths: std::collections::HashSet<_> =
        h.c.playlist
            .entries()
            .iter()
            .map(|e| e.path.clone())
            .collect();
    assert_eq!(paths.len(), 3);
    assert!(h.c.playlist.entries().iter().all(|e| e.slot.is_some()));
}

#[test]
fn playlist_survives_restart() {
    let mut h = Harness::new("queue");
    h.wav("01-a.wav");
    h.wav("02-b.wav");
    h.wav("03-c.wav");
    let file = h.dir.join("state").join("playlist.json");
    h.c.restore_queue(file.clone());
    h.c.handle(UiCommand::AddPaths {
        paths: vec![h.dir.clone()],
        index: None,
    });
    h.work(1);
    h.c.handle(UiCommand::Shuffle { spread: false });
    let order: Vec<PathBuf> =
        h.c.playlist
            .entries()
            .iter()
            .map(|e| e.path.clone())
            .collect();
    let (done, wait) = unbounded();
    h.c.handle(UiCommand::Shutdown { done });
    assert!(wait.try_recv().is_ok());
    assert!(file.exists());

    let mut back = Harness::new("queue-back");
    back.c.restore_queue(file);
    let restored: Vec<PathBuf> = back
        .c
        .playlist
        .entries()
        .iter()
        .map(|e| e.path.clone())
        .collect();
    assert_eq!(restored, order);
    assert!(back
        .c
        .playlist
        .entries()
        .iter()
        .all(|e| e.duration.is_some()));
}

#[test]
fn m3u_programs_and_categories_drive_the_clock() {
    let mut h = Harness::new("m3u-clock");
    let music = h.dir.join("Musique");
    std::fs::create_dir_all(&music).unwrap();
    for i in 0..5 {
        write_wav(&music.join(format!("t{i}.wav")), 44_100, 22_050);
    }
    let theme = h.dir.join("theme.m3u8");
    std::fs::write(&theme, "#EXTM3U\nMusique/t3.wav\nMusique/t4.wav\n").unwrap();
    let show = h.dir.join("emission.m3u");
    std::fs::write(
        &show,
        "Musique/t2.wav\nMusique/t0.wav\nMusique/absent.wav\n",
    )
    .unwrap();
    let mut cfg = AppConfig {
        rotation: blast_library::Rotation {
            categories: vec![blast_library::RotationCategory {
                id: "theme".into(),
                name: "Thème".into(),
                color: "#ff0000".into(),
                rule: blast_library::CategoryRule {
                    playlists: vec![theme.to_string_lossy().into_owned()],
                    ..Default::default()
                },
            }],
            clocks: vec![blast_library::Clock {
                id: "jour".into(),
                name: "Journée".into(),
                color: "#0000ff".into(),
                slots: vec!["theme".into()],
            }],
            programs: vec![blast_library::Program {
                id: "show".into(),
                name: "Spécial".into(),
                color: "#00ff00".into(),
                path: show.to_string_lossy().into_owned(),
            }],
            schedule: vec!["show".into(); blast_library::SCHEDULE_SLOTS],
            default_clock: Some("jour".into()),
        },
        ..Default::default()
    };
    cfg.automation.rules.enabled = false;
    h.c.handle(UiCommand::Config(Box::new(cfg)));
    loop {
        let w =
            h.c.work_rx
                .recv_timeout(Duration::from_secs(10))
                .expect("scan");
        let done = matches!(w, Work::LibraryDone { .. });
        h.c.on_work(w);
        if done {
            break;
        }
    }
    assert_eq!(h.last(EV_LIBRARY)["tracks"], 4);
    assert_eq!(h.last(EV_STATE)["clock"]["program"], true);
    assert!(h.c.rotation_fill());
    let names: Vec<String> =
        h.c.playlist
            .entries()
            .iter()
            .map(|e| e.path.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
    assert_eq!(names[..2], ["t2.wav".to_string(), "t0.wav".to_string()]);
    assert!(names[2] == "t3.wav" || names[2] == "t4.wav");
    let slots: Vec<bool> =
        h.c.playlist
            .entries()
            .iter()
            .map(|e| e.slot.as_ref().unwrap().fixed)
            .collect();
    assert_eq!(slots, [true, true, false]);
    assert!(h
        .notices("info")
        .iter()
        .any(|m| m.contains("Émission « Spécial » terminée")));
}

fn chat_msg(login: &str, text: &str) -> blast_live::ChatEvent {
    blast_live::ChatEvent::Message(blast_live::ChatMessage {
        id: format!("{login}-{text}"),
        platform: blast_live::Platform::Twitch,
        source: "twitch".into(),
        author: login.to_uppercase(),
        login: login.into(),
        color: String::new(),
        badges: Vec::new(),
        text: text.into(),
        action: false,
        own: false,
        donation: String::new(),
        at_ms: 0,
    })
}

#[test]
fn chat_requests_wait_for_approval() {
    let mut h = Harness::new("chat");
    let mut tracks = Vec::new();
    for (artist, title) in [
        ("D-Goth", "Void Relay"),
        ("D-Goth", "Orokin Dawn"),
        ("Black-Lab", "Nightfall"),
    ] {
        let path = h.wav(&format!("{artist} - {title}.wav"));
        tracks.push(blast_library::LibraryTrack {
            path: path.to_string_lossy().into_owned(),
            title: title.into(),
            artist: artist.into(),
            genre: String::new(),
            year: None,
            bpm: None,
            duration: Some(0.5),
            kind: Kind::Morceau,
            play_count: 0,
            last_played: None,
        });
    }
    h.c.rot.index = tracks
        .iter()
        .enumerate()
        .map(|(i, t)| (t.path.clone(), i))
        .collect();
    h.c.rot.tracks = tracks;
    let g = 0;

    h.c.on_chat(
        chat::ChatSource::Twitch,
        g,
        chat_msg("viewer", "!request d-goth void"),
    );
    h.c.on_chat(
        chat::ChatSource::Twitch,
        g,
        chat_msg("viewer", "!sr orokin"),
    );
    h.c.on_chat(
        chat::ChatSource::Twitch,
        g,
        chat_msg("viewer", "!request nightfall"),
    );
    h.c.on_chat(
        chat::ChatSource::Twitch,
        g,
        chat_msg("other", "!request metallica one"),
    );
    h.c.on_chat(
        chat::ChatSource::Twitch,
        g,
        chat_msg("other", "salut tout le monde"),
    );
    h.c.on_chat(chat::ChatSource::Twitch, g, chat_msg("other", "!np"));

    let sent = h.c.chat.sent.clone();
    assert!(
        sent[0].contains("@VIEWER demande reçue : « D-Goth - Void Relay »"),
        "{sent:?}"
    );
    assert!(sent[1].contains("« D-Goth - Orokin Dawn »"));
    assert!(sent[2].contains("déjà 2 demande(s)"));
    assert!(sent[3].contains("pas trouvé « metallica one »"));
    assert!(sent[4].contains("rien à l'antenne"));
    assert_eq!(sent.len(), 5);

    let state = h.last(EV_CHAT_STATE);
    let requests = state["requests"].as_array().unwrap();
    assert_eq!(requests.len(), 3);
    assert_eq!(requests[2]["path"], Value::Null);
    let first = requests[0]["id"].as_u64().unwrap();
    let missing = requests[2]["id"].as_u64().unwrap();

    h.c.handle(UiCommand::RequestAccept { id: first });
    let e = &h.c.playlist.entries()[0];
    assert_eq!(e.title, "Void Relay");
    assert!(e.slot.as_ref().unwrap().fixed);
    assert_eq!(e.slot.as_ref().unwrap().name, "Demande");
    assert!(h.c.chat.sent.last().unwrap().contains("passe bientôt"));
    h.c.handle(UiCommand::RequestReject { id: missing });
    assert_eq!(
        h.last(EV_CHAT_STATE)["requests"].as_array().unwrap().len(),
        1
    );

    assert_eq!(h.c.chat.history.len(), 6);
    h.c.on_chat(
        chat::ChatSource::Twitch,
        g,
        blast_live::ChatEvent::Clear {
            login: Some("other".into()),
        },
    );
    assert_eq!(h.c.chat.history.len(), 3);
    h.c.on_chat(
        chat::ChatSource::Twitch,
        g + 7,
        chat_msg("ghost", "ancien flux"),
    );
    assert_eq!(h.c.chat.history.len(), 3);

    h.c.chat.settings.cmd_request = false;
    h.c.on_chat(
        chat::ChatSource::Twitch,
        g,
        chat_msg("third", "!request nightfall"),
    );
    assert_eq!(h.c.chat.view.requests.len(), 1);
}

#[test]
fn ssn_replies_go_back_to_their_platform() {
    let mut h = Harness::new("chat-ssn");
    let path = h.wav("Void Relay.wav");
    h.c.rot.tracks = vec![blast_library::LibraryTrack {
        path: path.to_string_lossy().into_owned(),
        title: "Void Relay".into(),
        artist: "D-Goth".into(),
        genre: String::new(),
        year: None,
        bpm: None,
        duration: Some(0.5),
        kind: Kind::Morceau,
        play_count: 0,
        last_played: None,
    }];
    let ssn = chat::ChatSource::Ssn;
    h.c.chat.view.ssn.running = true;
    let tiktok = blast_live::ssn::parse_message(
        r#"{"chatname":"Lina","chatmessage":"!request void relay","type":"tiktok"}"#,
    )
    .unwrap();
    let yt = blast_live::ssn::parse_message(
        r#"{"chatname":"Max","chatmessage":"salut","type":"youtube"}"#,
    )
    .unwrap();
    h.c.on_chat(ssn, 0, tiktok);
    h.c.on_chat(ssn, 0, yt);
    assert_eq!(
        h.c.chat.sent,
        vec!["[tiktok] @Lina demande reçue : « D-Goth - Void Relay » (en attente de validation)"]
    );
    let id = h.c.chat.view.requests[0].id;
    h.c.handle(UiCommand::RequestAccept { id });
    assert!(h.c.chat.sent[1].starts_with("[tiktok] @Lina ta demande"));
    h.c.handle(UiCommand::ChatSay {
        text: "Bonsoir à tous".into(),
    });
    assert_eq!(h.c.chat.sent[2], "[*] Bonsoir à tous");
    h.c.chat.view.twitch.running = true;
    h.c.chat.view.twitch.writable = true;
    let dup = blast_live::ssn::parse_message(
        r#"{"chatname":"Twi","chatmessage":"doublon","type":"twitch"}"#,
    )
    .unwrap();
    h.c.on_chat(ssn, 0, dup);
    assert_eq!(h.c.chat.history.len(), 2);
    h.c.handle(UiCommand::ChatSay { text: "Re".into() });
    assert_eq!(&h.c.chat.sent[3..], ["Re", "[tiktok] Re", "[youtube] Re"]);
    h.c.chat.view.twitch.writable = false;
    h.c.handle(UiCommand::ChatSay { text: "Lu".into() });
    assert_eq!(
        &h.c.chat.sent[6..],
        ["[tiktok] Lu", "[youtube] Lu", "[twitch] Lu"]
    );
    h.c.on_chat(chat::ChatSource::Twitch, 0, chat_msg("anon", "!np"));
    assert!(h.c.chat.sent.last().unwrap().starts_with("[twitch] @ANON"));
    h.c.on_chat(
        ssn,
        5,
        blast_live::ssn::parse_message(r#"{"chatname":"Old","chatmessage":"x","type":"kick"}"#)
            .unwrap(),
    );
    assert_eq!(h.c.chat.history.len(), 3);
}
