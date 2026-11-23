use std::collections::{BTreeSet, VecDeque};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use blast_library::{Kind, LibraryTrack};
use blast_live::{
    parse_command, ChatCommand, ChatEvent, ChatMessage, SsnClient, SsnConfig, TwitchClient,
    TwitchConfig,
};
use serde::Serialize;

use super::views::{EV_CHAT_EVENT, EV_CHAT_HISTORY, EV_CHAT_STATE};
use super::{now_ms, Controller, Work};
use crate::config::ChatSettings;
use crate::playlist::{self, Entry, SlotTag};
use crate::secrets::{CHAT_KEY, SSN_KEY};

// [CTL-168]
const KEEP: usize = 300;
const NP_COOLDOWN: Duration = Duration::from_secs(15);
const SFX_COOLDOWN: Duration = Duration::from_secs(10);
const REQUEST_TAG: &str = "Demande";
const REQUEST_COLOR: &str = "#9146FF";

// [CTL-185]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ChatSource {
    Twitch,
    Ssn,
}

impl ChatSource {
    fn slot(self) -> usize {
        match self {
            ChatSource::Twitch => 0,
            ChatSource::Ssn => 1,
        }
    }
}

// [CTL-169]
#[derive(Debug, Clone, Serialize)]
pub struct SongRequest {
    pub id: u64,
    pub user: String,
    pub login: String,
    pub platform: String,
    pub via: ChatSource,
    pub query: String,
    pub path: Option<PathBuf>,
    pub artist: String,
    pub title: String,
    pub at_ms: u64,
}

// [CTL-186]
#[derive(Debug, Clone, Default, Serialize)]
pub struct SourceView {
    pub enabled: bool,
    pub running: bool,
    pub connected: bool,
    pub writable: bool,
    pub detail: String,
}

// [CTL-170]
#[derive(Debug, Clone, Default, Serialize)]
pub struct ChatView {
    pub channel: String,
    pub active: bool,
    pub twitch: SourceView,
    pub ssn: SourceView,
    pub requests: Vec<SongRequest>,
}

// [CTL-171]
pub struct ChatState {
    pub settings: ChatSettings,
    pub twitch: Option<TwitchClient>,
    pub ssn: Option<SsnClient>,
    pub view: ChatView,
    pub history: VecDeque<ChatEvent>,
    seen: BTreeSet<String>,
    next_request: u64,
    generation: [u64; 2],
    last_np: Option<Instant>,
    last_sfx: Option<Instant>,
    #[cfg(test)]
    pub sent: Vec<String>,
}

impl ChatState {
    pub fn new(settings: ChatSettings) -> Self {
        let mut s = Self {
            view: ChatView::default(),
            settings,
            twitch: None,
            ssn: None,
            history: VecDeque::new(),
            seen: BTreeSet::new(),
            next_request: 1,
            generation: [0; 2],
            last_np: None,
            last_sfx: None,
            #[cfg(test)]
            sent: Vec::new(),
        };
        s.sync_view();
        s
    }

    fn sync_view(&mut self) {
        self.view.channel = blast_live::twitch::normalize_channel(&self.settings.channel);
        self.view.twitch.enabled = self.settings.twitch_enabled;
        self.view.ssn.enabled = self.settings.ssn_enabled;
        self.view.active = self.twitch.is_some() || self.ssn.is_some();
    }

    fn source_mut(&mut self, s: ChatSource) -> &mut SourceView {
        match s {
            ChatSource::Twitch => &mut self.view.twitch,
            ChatSource::Ssn => &mut self.view.ssn,
        }
    }
}

// [CTL-172]
fn fold(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            'à' | 'â' | 'ä' | 'á' | 'ã' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'î' | 'ï' | 'í' => 'i',
            'ô' | 'ö' | 'ó' | 'õ' => 'o',
            'ù' | 'û' | 'ü' | 'ú' => 'u',
            'ç' => 'c',
            'ñ' => 'n',
            'ÿ' => 'y',
            c if c.is_alphanumeric() => c,
            _ => ' ',
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

// [CTL-173]
pub fn find_track(tracks: &[LibraryTrack], query: &str) -> Option<usize> {
    let wanted = fold(&query.to_lowercase());
    let words: Vec<&str> = wanted.split(' ').filter(|w| !w.is_empty()).collect();
    if words.is_empty() {
        return None;
    }
    let (artist_part, title_part) = match query.split_once(" - ") {
        Some((a, t)) => (fold(&a.to_lowercase()), fold(&t.to_lowercase())),
        None => (String::new(), wanted.clone()),
    };
    tracks
        .iter()
        .enumerate()
        .filter(|(_, t)| t.kind == Kind::Morceau)
        .filter_map(|(i, t)| {
            let artist = fold(&t.artist.to_lowercase());
            let title = fold(&t.title.to_lowercase());
            let hay = format!("{artist} {title}");
            let padded = format!(" {hay} ");
            if !words.iter().all(|w| padded.contains(&format!(" {w}"))) {
                return None;
            }
            let mut score = 0i32;
            if title == title_part {
                score += 40;
            } else if title.contains(&title_part) {
                score += 15;
            }
            if !artist_part.is_empty() && artist == artist_part {
                score += 30;
            }
            score += words
                .iter()
                .filter(|w| padded.contains(&format!(" {w} ")))
                .count() as i32
                * 3;
            score -= (hay.len() as i32) / 20;
            Some((score, i))
        })
        .max_by_key(|(score, i)| (*score, std::cmp::Reverse(*i)))
        .map(|(_, i)| i)
}

impl Controller {
    // [CTL-174]
    pub(super) fn apply_chat(&mut self, settings: ChatSettings) {
        if settings == self.chat.settings {
            return;
        }
        let reconnect = self.chat.view.active
            && (settings.channel != self.chat.settings.channel
                || settings.bot_login != self.chat.settings.bot_login
                || settings.twitch_enabled != self.chat.settings.twitch_enabled
                || settings.ssn_enabled != self.chat.settings.ssn_enabled);
        self.chat.settings = settings;
        self.chat.sync_view();
        if reconnect {
            self.chat_connect();
        } else {
            self.emit_chat_state();
        }
    }

    // [CTL-175]
    pub(super) fn chat_connect(&mut self) {
        self.chat_stop_clients();
        let mut started = false;
        if self.chat.settings.twitch_enabled {
            let token = if self.chat.settings.bot_login.trim().is_empty() {
                String::new()
            } else {
                self.secrets.get(CHAT_KEY).unwrap_or_default()
            };
            let cfg = TwitchConfig::new(
                &self.chat.settings.channel,
                &self.chat.settings.bot_login,
                &token,
            );
            if cfg.channel.is_empty() {
                self.notice(
                    "warn",
                    "Chat Twitch : indique ta chaîne dans Réglages → Chat.",
                );
            } else {
                let writable = cfg.writable();
                let tx = self.work_tx.clone();
                let generation = self.chat.generation[0];
                self.chat.twitch = Some(TwitchClient::spawn(cfg, move |e| {
                    let _ = tx.send(Work::Chat(ChatSource::Twitch, generation, e));
                }));
                *self.chat.source_mut(ChatSource::Twitch) = SourceView {
                    enabled: true,
                    running: true,
                    writable,
                    detail: "connexion…".into(),
                    ..Default::default()
                };
                started = true;
            }
        }
        if self.chat.settings.ssn_enabled {
            let session = self.secrets.get(SSN_KEY).unwrap_or_default();
            let cfg = SsnConfig::new(&session);
            if cfg.session.is_empty() {
                self.notice(
                    "warn",
                    "Social Stream Ninja : colle ton identifiant de session dans Réglages → Chat.",
                );
            } else {
                let tx = self.work_tx.clone();
                let generation = self.chat.generation[1];
                self.chat.ssn = Some(SsnClient::spawn(cfg, move |e| {
                    let _ = tx.send(Work::Chat(ChatSource::Ssn, generation, e));
                }));
                *self.chat.source_mut(ChatSource::Ssn) = SourceView {
                    enabled: true,
                    running: true,
                    detail: "connexion…".into(),
                    ..Default::default()
                };
                started = true;
            }
        }
        if !started && !self.chat.settings.twitch_enabled && !self.chat.settings.ssn_enabled {
            self.notice(
                "warn",
                "Aucune source de chat activée : choisis-en une dans Réglages → Chat.",
            );
        }
        self.chat.sync_view();
        self.emit_chat_state();
    }

    fn chat_stop_clients(&mut self) {
        self.chat.generation[0] += 1;
        self.chat.generation[1] += 1;
        if let Some(mut c) = self.chat.twitch.take() {
            c.stop();
        }
        if let Some(mut c) = self.chat.ssn.take() {
            c.stop();
        }
        for s in [ChatSource::Twitch, ChatSource::Ssn] {
            let v = self.chat.source_mut(s);
            v.running = false;
            v.connected = false;
            v.detail.clear();
        }
    }

    // [CTL-176]
    pub(super) fn chat_disconnect(&mut self) {
        self.chat_stop_clients();
        self.chat.sync_view();
        self.emit_chat_state();
    }

    fn twitch_writable(&self) -> bool {
        self.chat.view.twitch.running && self.chat.view.twitch.writable
    }

    fn source_ready(&self, s: ChatSource) -> bool {
        if cfg!(test) {
            return true;
        }
        match s {
            ChatSource::Twitch => self.chat.view.twitch.connected && self.chat.view.twitch.writable,
            ChatSource::Ssn => self.chat.view.ssn.connected,
        }
    }

    // [CTL-177]
    fn chat_reply(&mut self, via: ChatSource, platform: &str, text: &str) {
        let via =
            if via == ChatSource::Twitch && !self.twitch_writable() && self.chat.view.ssn.running {
                ChatSource::Ssn
            } else {
                via
            };
        if !self.source_ready(via) {
            return;
        }
        match via {
            ChatSource::Twitch => {
                #[cfg(test)]
                self.chat.sent.push(text.to_string());
                if let Some(c) = &self.chat.twitch {
                    c.say(text);
                }
            }
            ChatSource::Ssn => {
                #[cfg(test)]
                self.chat.sent.push(format!("[{platform}] {text}"));
                if let Some(c) = &self.chat.ssn {
                    c.say(text, Some(platform));
                }
            }
        }
    }

    // [CTL-187]
    pub(super) fn chat_broadcast(&mut self, text: &str) {
        let text = text.trim();
        if text.is_empty() {
            return;
        }
        let twitch_direct = self.twitch_writable();
        if twitch_direct && self.source_ready(ChatSource::Twitch) {
            #[cfg(test)]
            self.chat.sent.push(text.to_string());
            if let Some(c) = &self.chat.twitch {
                c.say(text);
            }
        }
        if self.chat.view.ssn.running && self.source_ready(ChatSource::Ssn) {
            let targets: Vec<Option<String>> = if twitch_direct {
                self.chat
                    .seen
                    .iter()
                    .filter(|p| p.as_str() != "twitch")
                    .map(|p| Some(p.clone()))
                    .collect()
            } else if self.chat.view.twitch.running {
                let mut t: Vec<Option<String>> =
                    self.chat.seen.iter().map(|p| Some(p.clone())).collect();
                if !self.chat.seen.contains("twitch") {
                    t.push(Some("twitch".into()));
                }
                t
            } else {
                vec![None]
            };
            for t in targets {
                #[cfg(test)]
                self.chat
                    .sent
                    .push(format!("[{}] {text}", t.as_deref().unwrap_or("*")));
                if let Some(c) = &self.chat.ssn {
                    c.say(text, t.as_deref());
                }
            }
        }
    }

    // [CTL-178]
    pub(super) fn on_chat(&mut self, from: ChatSource, generation: u64, e: ChatEvent) {
        if generation != self.chat.generation[from.slot()] {
            return;
        }
        match &e {
            ChatEvent::Status {
                connected,
                writable,
                detail,
            } => {
                let v = self.chat.source_mut(from);
                v.connected = *connected;
                v.writable = *writable;
                v.detail = detail.clone();
                self.emit_chat_state();
                return;
            }
            ChatEvent::Message(m) if from == ChatSource::Ssn => {
                if m.source == "twitch" && self.chat.view.twitch.running {
                    return;
                }
                if !m.source.is_empty() {
                    self.chat.seen.insert(m.source.clone());
                }
                self.chat.history.push_back(e.clone());
            }
            ChatEvent::Clear { login } => match login {
                Some(l) => self.chat.history.retain(
                    |h| !matches!(h, ChatEvent::Message(m) if m.login.eq_ignore_ascii_case(l)),
                ),
                None => self.chat.history.clear(),
            },
            ChatEvent::Delete { id } => self
                .chat
                .history
                .retain(|h| !matches!(h, ChatEvent::Message(m) if &m.id == id)),
            ChatEvent::Message(_) | ChatEvent::Notice { .. } => {
                self.chat.history.push_back(e.clone());
            }
        }
        while self.chat.history.len() > KEEP {
            self.chat.history.pop_front();
        }
        self.ui
            .emit(EV_CHAT_EVENT, serde_json::to_value(&e).unwrap_or_default());
        if let ChatEvent::Message(m) = e {
            if !m.own {
                self.chat_command(&m, from);
            }
        }
    }

    // [CTL-179]
    fn chat_command(&mut self, m: &ChatMessage, from: ChatSource) {
        let Some(cmd) = parse_command(&m.text) else {
            return;
        };
        match cmd {
            ChatCommand::NowPlaying if self.chat.settings.cmd_np => {
                if self.chat.last_np.is_some_and(|t| t.elapsed() < NP_COOLDOWN) {
                    return;
                }
                self.chat.last_np = Some(Instant::now());
                let song = self.now_playing();
                let song = if song.is_empty() {
                    self.cast.song.clone()
                } else {
                    song
                };
                let reply = if song.is_empty() {
                    format!("@{} rien à l'antenne pour l'instant.", m.author)
                } else {
                    format!("@{} 🎵 En cours : {song}", m.author)
                };
                self.chat_reply(from, &m.source, &reply);
            }
            ChatCommand::Request(query) if self.chat.settings.cmd_request => {
                self.chat_request(m, from, &query)
            }
            ChatCommand::Sfx(name) if self.chat.settings.cmd_sfx => {
                if self
                    .chat
                    .last_sfx
                    .is_some_and(|t| t.elapsed() < SFX_COOLDOWN)
                {
                    return;
                }
                let wanted = fold(&name.to_lowercase());
                let pad = self
                    .pads
                    .pads
                    .iter()
                    .filter(|(_, p)| p.track.is_some() && !p.name.trim().is_empty())
                    .find(|(_, p)| fold(&p.name.to_lowercase()) == wanted)
                    .map(|(i, _)| *i);
                if let Some(index) = pad {
                    self.chat.last_sfx = Some(Instant::now());
                    self.pad_play(index);
                }
            }
            _ => {}
        }
    }

    // [CTL-180]
    fn chat_request(&mut self, m: &ChatMessage, from: ChatSource, query: &str) {
        let pending = self
            .chat
            .view
            .requests
            .iter()
            .filter(|r| r.login == m.login && r.path.is_some())
            .count() as u32;
        let limit = self.chat.settings.request_limit.max(1);
        if pending >= limit {
            self.chat_reply(
                from,
                &m.source,
                &format!(
                    "@{} tu as déjà {pending} demande(s) en attente, patience !",
                    m.author
                ),
            );
            return;
        }
        let found = find_track(&self.rot.tracks, query).map(|i| self.rot.tracks[i].clone());
        let id = self.chat.next_request;
        self.chat.next_request += 1;
        let request = SongRequest {
            id,
            user: m.author.clone(),
            login: m.login.clone(),
            platform: m.source.clone(),
            via: from,
            query: query.to_string(),
            path: found.as_ref().map(|t| PathBuf::from(&t.path)),
            artist: found.as_ref().map(|t| t.artist.clone()).unwrap_or_default(),
            title: found.as_ref().map(|t| t.title.clone()).unwrap_or_default(),
            at_ms: now_ms(),
        };
        let reply = match &found {
            Some(t) => format!(
                "@{} demande reçue : {} (en attente de validation)",
                m.author,
                song_label(&t.artist, &t.title)
            ),
            None => format!(
                "@{} je n'ai pas trouvé « {query} » dans la bibliothèque.",
                m.author
            ),
        };
        self.chat.view.requests.push(request);
        self.emit_chat_state();
        self.chat_reply(from, &m.source, &reply);
    }

    // [CTL-181]
    pub(super) fn request_accept(&mut self, id: u64) {
        let Some(pos) = self.chat.view.requests.iter().position(|r| r.id == id) else {
            return;
        };
        let r = self.chat.view.requests.remove(pos);
        let Some(path) = r.path.clone() else {
            self.emit_chat_state();
            return;
        };
        if !path.is_file() {
            self.notice(
                "warn",
                format!("Demande de {} : fichier introuvable.", r.user),
            );
            self.emit_chat_state();
            return;
        }
        let tag = SlotTag {
            name: REQUEST_TAG.into(),
            color: REQUEST_COLOR.into(),
            fixed: true,
        };
        let known = self.rot.index.get(&*path.to_string_lossy()).copied();
        let entry: Entry = match known {
            Some(i) => playlist::from_library(&self.rot.tracks[i], tag),
            None => Entry {
                slot: Some(tag),
                ..playlist::make_entry(&path, &self.categories)
            },
        };
        self.playlist.insert(vec![entry], Some(0));
        self.emit_playlist();
        self.emit_chat_state();
        let text = format!(
            "@{} ta demande {} passe bientôt !",
            r.user,
            song_label(&r.artist, &r.title)
        );
        self.chat_reply(r.via, &r.platform, &text);
    }

    // [CTL-182]
    pub(super) fn request_reject(&mut self, id: u64) {
        self.chat.view.requests.retain(|r| r.id != id);
        self.emit_chat_state();
    }

    // [CTL-183]
    pub(super) fn chat_announce(&mut self, song: &str) {
        if self.chat.settings.announce && self.chat.view.active {
            self.chat_broadcast(&format!("🎵 En cours : {song}"));
        }
    }

    // [CTL-184]
    pub(super) fn emit_chat_state(&self) {
        self.ui.emit(
            EV_CHAT_STATE,
            serde_json::to_value(&self.chat.view).unwrap_or_default(),
        );
    }

    pub(super) fn emit_chat_history(&self) {
        let items: Vec<&ChatEvent> = self.chat.history.iter().collect();
        self.ui
            .emit(EV_CHAT_HISTORY, serde_json::json!({ "items": items }));
    }
}

fn song_label(artist: &str, title: &str) -> String {
    match (artist.trim(), title.trim()) {
        ("", t) => format!("« {t} »"),
        (a, t) => format!("« {a} - {t} »"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(artist: &str, title: &str) -> LibraryTrack {
        LibraryTrack {
            path: format!("/m/{artist} - {title}.mp3"),
            title: title.into(),
            artist: artist.into(),
            genre: String::new(),
            year: None,
            bpm: None,
            duration: Some(200.0),
            kind: Kind::Morceau,
            play_count: 0,
            last_played: None,
        }
    }

    #[test]
    fn finds_tracks_loosely() {
        let lib = vec![
            track("Nine Inch Nails", "Sin"),
            track("Nine Inch Nails", "Something I Can Never Have"),
            track("Röyksopp", "Eple"),
            track("Mylène Farmer", "Désenchantée"),
            track("Sinéad O'Connor", "Nothing Compares 2 U"),
        ];
        assert_eq!(find_track(&lib, "Nine Inch Nails - Sin"), Some(0));
        assert_eq!(find_track(&lib, "nin something never"), Some(1));
        assert_eq!(find_track(&lib, "ple"), None);
        assert_eq!(find_track(&lib, "something i can never"), Some(1));
        assert_eq!(find_track(&lib, "mylene farmer desenchantee"), Some(3));
        assert_eq!(find_track(&lib, "nothing compares"), Some(4));
        assert_eq!(find_track(&lib, "  "), None);
        assert_eq!(find_track(&lib, "Metallica One"), None);
    }
}
