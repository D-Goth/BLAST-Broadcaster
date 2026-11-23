use std::path::{Path, PathBuf};
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use xmltree::{Element, EmitterConfig, XMLNode};

// [ICE-03]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ServerLimits {
    pub clients: u32,
    pub sources: u32,
    pub queue_size: u32,
    pub client_timeout: u32,
    pub header_timeout: u32,
    pub source_timeout: u32,
    pub burst_size: u32,
}

impl Default for ServerLimits {
    fn default() -> Self {
        Self {
            clients: 100,
            sources: 2,
            queue_size: 524_288,
            client_timeout: 30,
            header_timeout: 15,
            source_timeout: 10,
            burst_size: 65_535,
        }
    }
}

// [ICE-04]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ServerAuth {
    pub source_password: String,
    pub relay_password: String,
    pub admin_user: String,
    pub admin_password: String,
}

// [ICE-05]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ListenSocket {
    pub key: Option<usize>,
    pub port: u16,
    pub bind_address: String,
    pub shoutcast_mount: String,
    pub ssl: bool,
}

// [ICE-06]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Mount {
    pub key: Option<usize>,
    pub mount_name: String,
    pub username: String,
    pub password: String,
    pub max_listeners: u32,
    pub fallback_mount: String,
    pub fallback_override: bool,
    pub public: Option<bool>,
    pub stream_name: String,
    pub stream_description: String,
    pub stream_url: String,
    pub genre: String,
    pub bitrate: String,
    pub hidden: bool,
}

// [ICE-07]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Paths {
    pub basedir: String,
    pub logdir: String,
    pub webroot: String,
    pub adminroot: String,
    pub pidfile: String,
    pub ssl_certificate: String,
}

// [ICE-08]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Logging {
    pub accesslog: String,
    pub errorlog: String,
    pub loglevel: u8,
    pub logsize: u32,
    pub logarchive: bool,
}

impl Default for Logging {
    fn default() -> Self {
        Self {
            accesslog: "access.log".into(),
            errorlog: "error.log".into(),
            loglevel: 3,
            logsize: 10_000,
            logarchive: false,
        }
    }
}

// [ICE-09]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct IceConfig {
    pub location: String,
    pub admin: String,
    pub hostname: String,
    pub limits: ServerLimits,
    pub auth: ServerAuth,
    pub sockets: Vec<ListenSocket>,
    pub mounts: Vec<Mount>,
    pub paths: Paths,
    pub logging: Logging,
}

// [ICE-10]
const STARTER: &str = r#"<icecast>
    <location>Terre</location>
    <admin>icemaster@localhost</admin>
    <hostname>localhost</hostname>
    <limits></limits>
    <authentication></authentication>
    <listen-socket><port>8000</port></listen-socket>
    <http-headers>
        <header name="Access-Control-Allow-Origin" value="*" />
    </http-headers>
    <fileserve>1</fileserve>
    <paths>
        <logdir>./log</logdir>
        <webroot>./web</webroot>
        <adminroot>./admin</adminroot>
        <alias source="/" destination="/status.xsl"/>
    </paths>
    <logging></logging>
    <security>
        <chroot>0</chroot>
    </security>
</icecast>"#;

// [ICE-11]
#[derive(Debug, Clone, PartialEq)]
pub struct IceDocument {
    root: Element,
}

fn named<'a>(name: &'a str) -> impl Fn(&XMLNode) -> bool + 'a {
    move |n| matches!(n, XMLNode::Element(e) if e.name == name)
}

fn text(e: &Element, name: &str) -> String {
    e.get_child(name)
        .and_then(|c| c.get_text())
        .map(|t| t.trim().to_string())
        .unwrap_or_default()
}

fn number<T: FromStr>(e: &Element, name: &str, default: T) -> T {
    text(e, name).parse().unwrap_or(default)
}

fn flag(e: &Element, name: &str) -> Option<bool> {
    e.get_child(name)?;
    Some(matches!(
        text(e, name).to_ascii_lowercase().as_str(),
        "1" | "true" | "yes"
    ))
}

fn child_mut<'a>(e: &'a mut Element, name: &str) -> &'a mut Element {
    let at = match e.children.iter().position(named(name)) {
        Some(i) => i,
        None => {
            e.children.push(XMLNode::Element(Element::new(name)));
            e.children.len() - 1
        }
    };
    match &mut e.children[at] {
        XMLNode::Element(c) => c,
        _ => unreachable!(),
    }
}

fn set(e: &mut Element, name: &str, value: &str) {
    let value = value.trim();
    if value.is_empty() {
        e.children.retain(|n| !named(name)(n));
        return;
    }
    child_mut(e, name).children = vec![XMLNode::Text(value.to_string())];
}

// [ICE-44]
fn modern(r: &Element) -> bool {
    let tls_context = r
        .get_child("security")
        .is_some_and(|s| s.get_child("tls-context").is_some());
    let tls_socket = elements(r, "listen-socket").any(|s| s.get_child("tls").is_some());
    let level = r
        .get_child("logging")
        .map(|l| text(l, "loglevel"))
        .unwrap_or_default();
    tls_context || tls_socket || (!level.is_empty() && level.parse::<u8>().is_err())
}

// [ICE-45]
fn level_from(value: &str, default: u8) -> u8 {
    match value.trim().to_ascii_lowercase().as_str() {
        "error" | "eror" => 1,
        "warning" | "warn" => 2,
        "information" | "info" => 3,
        "debug" | "dbug" => 4,
        v => v.parse().unwrap_or(default),
    }
}

fn level_word(level: u8) -> &'static str {
    match level {
        1 => "error",
        2 => "warning",
        4 => "debug",
        _ => "information",
    }
}

fn socket_tls(s: &Element) -> bool {
    let tls = text(s, "tls").to_ascii_lowercase();
    flag(s, "ssl").unwrap_or(false) || (!tls.is_empty() && tls != "disabled")
}

fn set_flag(e: &mut Element, name: &str, value: bool) {
    set(e, name, if value { "1" } else { "0" });
}

fn elements<'a>(e: &'a Element, name: &'a str) -> impl Iterator<Item = &'a Element> + 'a {
    e.children.iter().filter_map(move |n| match n {
        XMLNode::Element(c) if c.name == name => Some(c),
        _ => None,
    })
}

fn replace_all(parent: &mut Element, name: &str, items: Vec<Element>) {
    let first = parent.children.iter().position(named(name));
    parent.children.retain(|n| !named(name)(n));
    let at = first
        .unwrap_or(parent.children.len())
        .min(parent.children.len());
    parent
        .children
        .splice(at..at, items.into_iter().map(XMLNode::Element));
}

impl IceDocument {
    pub fn parse(text: &str) -> Result<Self, String> {
        let root = Element::parse(text.as_bytes()).map_err(|e| format!("XML invalide : {e}"))?;
        if root.name != "icecast" {
            return Err(format!(
                "ce fichier n'est pas une configuration Icecast (balise racine <{}>)",
                root.name
            ));
        }
        Ok(Self { root })
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("lecture de {} impossible : {e}", path.display()))?;
        Self::parse(&text)
    }

    pub fn starter() -> Self {
        let mut doc = Self::parse(STARTER).unwrap_or_else(|_| Self {
            root: Element::new("icecast"),
        });
        let mut cfg = doc.read();
        cfg.limits = ServerLimits::default();
        cfg.logging = Logging::default();
        cfg.auth = ServerAuth {
            source_password: crate::random_password(16),
            relay_password: crate::random_password(16),
            admin_user: "admin".into(),
            admin_password: crate::random_password(16),
        };
        doc.apply(&cfg);
        doc
    }

    // [ICE-12]
    pub fn read(&self) -> IceConfig {
        let r = &self.root;
        let empty = Element::new("");
        let limits = r.get_child("limits").unwrap_or(&empty);
        let auth = r.get_child("authentication").unwrap_or(&empty);
        let paths = r.get_child("paths").unwrap_or(&empty);
        let logging = r.get_child("logging").unwrap_or(&empty);
        let d = ServerLimits::default();
        let ld = Logging::default();
        IceConfig {
            location: text(r, "location"),
            admin: text(r, "admin"),
            hostname: text(r, "hostname"),
            limits: ServerLimits {
                clients: number(limits, "clients", d.clients),
                sources: number(limits, "sources", d.sources),
                queue_size: number(limits, "queue-size", d.queue_size),
                client_timeout: number(limits, "client-timeout", d.client_timeout),
                header_timeout: number(limits, "header-timeout", d.header_timeout),
                source_timeout: number(limits, "source-timeout", d.source_timeout),
                burst_size: number(limits, "burst-size", d.burst_size),
            },
            auth: ServerAuth {
                source_password: text(auth, "source-password"),
                relay_password: text(auth, "relay-password"),
                admin_user: text(auth, "admin-user"),
                admin_password: text(auth, "admin-password"),
            },
            sockets: elements(r, "listen-socket")
                .enumerate()
                .map(|(i, s)| ListenSocket {
                    key: Some(i),
                    port: number(s, "port", 0),
                    bind_address: text(s, "bind-address"),
                    shoutcast_mount: text(s, "shoutcast-mount"),
                    ssl: socket_tls(s),
                })
                .collect(),
            mounts: elements(r, "mount")
                .enumerate()
                .map(|(i, m)| Mount {
                    key: Some(i),
                    mount_name: text(m, "mount-name"),
                    username: text(m, "username"),
                    password: text(m, "password"),
                    max_listeners: text(m, "max-listeners")
                        .parse::<i64>()
                        .ok()
                        .filter(|n| *n > 0)
                        .map_or(0, |n| n.min(u32::MAX as i64) as u32),
                    fallback_mount: text(m, "fallback-mount"),
                    fallback_override: flag(m, "fallback-override").unwrap_or(false),
                    public: flag(m, "public"),
                    stream_name: text(m, "stream-name"),
                    stream_description: text(m, "stream-description"),
                    stream_url: text(m, "stream-url"),
                    genre: text(m, "genre"),
                    bitrate: text(m, "bitrate"),
                    hidden: flag(m, "hidden").unwrap_or(false),
                })
                .collect(),
            paths: Paths {
                basedir: text(paths, "basedir"),
                logdir: text(paths, "logdir"),
                webroot: text(paths, "webroot"),
                adminroot: text(paths, "adminroot"),
                pidfile: text(paths, "pidfile"),
                ssl_certificate: {
                    let legacy = text(paths, "ssl-certificate");
                    if legacy.is_empty() {
                        r.get_child("security")
                            .and_then(|s| s.get_child("tls-context"))
                            .map(|t| text(t, "tls-certificate"))
                            .unwrap_or_default()
                    } else {
                        legacy
                    }
                },
            },
            logging: Logging {
                accesslog: text(logging, "accesslog"),
                errorlog: text(logging, "errorlog"),
                loglevel: level_from(&text(logging, "loglevel"), ld.loglevel),
                logsize: number(logging, "logsize", ld.logsize),
                logarchive: flag(logging, "logarchive").unwrap_or(false),
            },
        }
    }

    // [ICE-13]
    pub fn apply(&mut self, cfg: &IceConfig) {
        let modern = modern(&self.root);
        let r = &mut self.root;
        set(r, "location", &cfg.location);
        set(r, "admin", &cfg.admin);
        set(r, "hostname", &cfg.hostname);

        let l = child_mut(r, "limits");
        let lim = &cfg.limits;
        for (name, v) in [
            ("clients", lim.clients),
            ("sources", lim.sources),
            ("queue-size", lim.queue_size),
            ("client-timeout", lim.client_timeout),
            ("header-timeout", lim.header_timeout),
            ("source-timeout", lim.source_timeout),
            ("burst-size", lim.burst_size),
        ] {
            set(l, name, &v.to_string());
        }

        let a = child_mut(r, "authentication");
        set(a, "source-password", &cfg.auth.source_password);
        set(a, "relay-password", &cfg.auth.relay_password);
        set(a, "admin-user", &cfg.auth.admin_user);
        set(a, "admin-password", &cfg.auth.admin_password);

        let old_sockets: Vec<Element> = elements(r, "listen-socket").cloned().collect();
        let sockets = cfg
            .sockets
            .iter()
            .map(|s| {
                let mut e = s
                    .key
                    .and_then(|k| old_sockets.get(k).cloned())
                    .unwrap_or_else(|| Element::new("listen-socket"));
                set(&mut e, "port", &s.port.to_string());
                set(&mut e, "bind-address", &s.bind_address);
                set(&mut e, "shoutcast-mount", &s.shoutcast_mount);
                let had_tls = socket_tls(&e);
                if !s.ssl {
                    set(&mut e, "ssl", "");
                    set(&mut e, "tls", "");
                } else if modern {
                    set(&mut e, "ssl", "");
                    if !had_tls {
                        set(&mut e, "tls", "auto_no_plain");
                    }
                } else if !had_tls {
                    set_flag(&mut e, "ssl", true);
                }
                e
            })
            .collect();
        replace_all(r, "listen-socket", sockets);

        let old_mounts: Vec<Element> = elements(r, "mount").cloned().collect();
        let mounts = cfg
            .mounts
            .iter()
            .map(|m| {
                let mut e = m
                    .key
                    .and_then(|k| old_mounts.get(k).cloned())
                    .unwrap_or_else(|| {
                        let mut e = Element::new("mount");
                        e.attributes.insert("type".into(), "normal".into());
                        e
                    });
                set(&mut e, "mount-name", &m.mount_name);
                set(&mut e, "username", &m.username);
                set(&mut e, "password", &m.password);
                let max = if m.max_listeners > 0 {
                    m.max_listeners.to_string()
                } else {
                    String::new()
                };
                set(&mut e, "max-listeners", &max);
                set(&mut e, "fallback-mount", &m.fallback_mount);
                if m.fallback_mount.trim().is_empty() {
                    set(&mut e, "fallback-override", "");
                } else {
                    set_flag(&mut e, "fallback-override", m.fallback_override);
                }
                match m.public {
                    Some(p) => set_flag(&mut e, "public", p),
                    None => set(&mut e, "public", ""),
                }
                set(&mut e, "stream-name", &m.stream_name);
                set(&mut e, "stream-description", &m.stream_description);
                set(&mut e, "stream-url", &m.stream_url);
                set(&mut e, "genre", &m.genre);
                set(&mut e, "bitrate", &m.bitrate);
                if m.hidden {
                    set_flag(&mut e, "hidden", true);
                } else {
                    set(&mut e, "hidden", "");
                }
                e
            })
            .collect();
        replace_all(r, "mount", mounts);

        let p = child_mut(r, "paths");
        set(p, "basedir", &cfg.paths.basedir);
        set(p, "logdir", &cfg.paths.logdir);
        set(p, "webroot", &cfg.paths.webroot);
        set(p, "adminroot", &cfg.paths.adminroot);
        set(p, "pidfile", &cfg.paths.pidfile);
        if modern {
            set(p, "ssl-certificate", "");
            let sec = child_mut(r, "security");
            let ctx = child_mut(sec, "tls-context");
            set(ctx, "tls-certificate", &cfg.paths.ssl_certificate);
        } else {
            set(p, "ssl-certificate", &cfg.paths.ssl_certificate);
        }

        let g = child_mut(r, "logging");
        set(g, "accesslog", &cfg.logging.accesslog);
        set(g, "errorlog", &cfg.logging.errorlog);
        if modern {
            set(g, "loglevel", level_word(cfg.logging.loglevel));
        } else {
            set(g, "loglevel", &cfg.logging.loglevel.to_string());
        }
        set(g, "logsize", &cfg.logging.logsize.to_string());
        if cfg.logging.logarchive || g.get_child("logarchive").is_some() {
            let on = cfg.logging.logarchive;
            set(
                g,
                "logarchive",
                match (modern, on) {
                    (true, true) => "true",
                    (true, false) => "false",
                    (false, true) => "1",
                    (false, false) => "0",
                },
            );
        }
    }

    // [ICE-14]
    pub fn to_xml(&self) -> String {
        let mut out = Vec::new();
        let config = EmitterConfig::new()
            .perform_indent(true)
            .indent_string("    ")
            .write_document_declaration(false);
        if self.root.write_with_config(&mut out, config).is_err() {
            return String::new();
        }
        let mut text = String::from_utf8(out).unwrap_or_default();
        text.push('\n');
        text
    }
}

// [ICE-15]
impl IceConfig {
    pub fn validate(&self) -> Result<Vec<String>, String> {
        let mut warnings = Vec::new();
        if self.hostname.trim().is_empty() {
            return Err("Indique le nom d'hôte du serveur (hostname).".into());
        }
        if self.sockets.is_empty() {
            return Err("Il faut au moins un port d'écoute.".into());
        }
        let mut taken: Vec<(u16, &str)> = Vec::new();
        for s in &self.sockets {
            if s.port == 0 {
                return Err("Un port d'écoute vaut 0 : choisis un port entre 1 et 65535.".into());
            }
            let bind = s.bind_address.trim();
            let mut ports = vec![s.port];
            if !s.shoutcast_mount.trim().is_empty() {
                if !s.shoutcast_mount.starts_with('/') {
                    return Err(format!(
                        "Le point de montage Shoutcast du port {} doit commencer par « / ».",
                        s.port
                    ));
                }
                if s.port == u16::MAX {
                    return Err("Le port 65535 ne peut pas avoir de compatibilité Shoutcast (elle utilise le port suivant).".into());
                }
                ports.push(s.port + 1);
            }
            for p in ports {
                if taken
                    .iter()
                    .any(|(tp, tb)| *tp == p && (tb.is_empty() || bind.is_empty() || *tb == bind))
                {
                    return Err(format!("Le port {p} est utilisé deux fois."));
                }
                taken.push((p, bind));
            }
        }
        if self.auth.source_password.trim().is_empty() {
            return Err("Le mot de passe des sources ne peut pas être vide.".into());
        }
        if self.auth.admin_user.trim().is_empty() || self.auth.admin_password.trim().is_empty() {
            return Err("Indique l'identifiant et le mot de passe admin.".into());
        }
        for (label, pw) in [
            ("sources", &self.auth.source_password),
            ("relais", &self.auth.relay_password),
            ("admin", &self.auth.admin_password),
        ] {
            if pw == "hackme" {
                warnings.push(format!(
                    "Le mot de passe {label} est encore « hackme », celui d'origine : change-le."
                ));
            }
        }
        if self.limits.clients == 0 || self.limits.sources == 0 {
            return Err("Les limites de clients et de sources doivent être d'au moins 1.".into());
        }
        if self.limits.queue_size == 0 || self.limits.client_timeout == 0 {
            return Err("La taille de file et les délais doivent être supérieurs à 0.".into());
        }
        if self.limits.burst_size > self.limits.queue_size {
            return Err(
                "La rafale de connexion (burst) ne peut pas dépasser la taille de file.".into(),
            );
        }
        if !(1..=4).contains(&self.logging.loglevel) {
            return Err("Le niveau de journal va de 1 (erreurs) à 4 (débogage).".into());
        }
        let mut names: Vec<&str> = Vec::new();
        for m in &self.mounts {
            let name = m.mount_name.trim();
            if !name.starts_with('/') || name.len() < 2 || name.contains(char::is_whitespace) {
                return Err(format!(
                    "Point de montage « {name} » invalide : il commence par « / » et ne contient pas d'espace."
                ));
            }
            if names.contains(&name) {
                return Err(format!("Le point de montage {name} existe deux fois."));
            }
            names.push(name);
            let fb = m.fallback_mount.trim();
            if !fb.is_empty() && (!fb.starts_with('/') || fb == name) {
                return Err(format!(
                    "Le secours de {name} doit être un autre point de montage commençant par « / »."
                ));
            }
        }
        for m in &self.mounts {
            let fb = m.fallback_mount.trim();
            if !fb.is_empty() && !names.contains(&fb) && !fb.contains('.') {
                warnings.push(format!(
                    "Le secours {fb} de {} n'est pas déclaré : il ne fonctionnera que si une source s'y connecte.",
                    m.mount_name
                ));
            }
        }
        Ok(warnings)
    }

    // [ICE-16]
    pub fn rebase(&mut self, base: &Path, log_dir: &Path) {
        let absolute = |p: &str| -> String {
            let trimmed = p.trim();
            if trimmed.is_empty() || Path::new(trimmed).is_absolute() {
                return trimmed.to_string();
            }
            let rel = trimmed.trim_start_matches("./").trim_start_matches(".\\");
            base.join(rel).display().to_string()
        };
        self.paths.basedir = absolute(&self.paths.basedir);
        self.paths.webroot = absolute(&self.paths.webroot);
        self.paths.adminroot = absolute(&self.paths.adminroot);
        self.paths.ssl_certificate = absolute(&self.paths.ssl_certificate);
        self.paths.logdir = log_dir.display().to_string();
        if !self.paths.pidfile.trim().is_empty() && Path::new(&self.paths.pidfile).is_relative() {
            let name = Path::new(&self.paths.pidfile)
                .file_name()
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("icecast.pid"));
            self.paths.pidfile = log_dir.join(name).display().to_string();
        }
    }

    // [ICE-17]
    pub fn resolve(&self, workdir: &Path, value: &str) -> PathBuf {
        let p = Path::new(value.trim());
        if p.is_absolute() {
            return p.to_path_buf();
        }
        let base = if self.paths.basedir.trim().is_empty() {
            workdir.to_path_buf()
        } else {
            workdir.join(self.paths.basedir.trim())
        };
        base.join(value.trim().trim_start_matches("./"))
    }

    pub fn log_file(&self, workdir: &Path, which: &str) -> Option<PathBuf> {
        let name = match which {
            "access" => &self.logging.accesslog,
            "error" => &self.logging.errorlog,
            _ => return None,
        };
        if name.trim().is_empty() || self.paths.logdir.trim().is_empty() {
            return None;
        }
        Some(self.resolve(workdir, &self.paths.logdir).join(name.trim()))
    }

    pub fn first_port(&self) -> Option<u16> {
        self.sockets.iter().map(|s| s.port).find(|p| *p > 0)
    }
}

// [ICE-18]
pub fn is_writable(path: &Path) -> bool {
    if path.exists() {
        return std::fs::OpenOptions::new().append(true).open(path).is_ok();
    }
    let Some(dir) = path.parent() else {
        return false;
    };
    let probe = dir.join(format!(".blast-probe-{}", std::process::id()));
    let ok = std::fs::write(&probe, b"").is_ok();
    let _ = std::fs::remove_file(&probe);
    ok
}

// [ICE-19]
pub fn write_file(path: &Path, xml: &str, keep: usize) -> Result<Option<PathBuf>, String> {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .ok_or("chemin de configuration invalide")?;
    let dir = path.parent().map(PathBuf::from).unwrap_or_default();
    if !dir.as_os_str().is_empty() {
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    }
    let mut backup = None;
    if path.exists() {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let b = dir.join(format!("{name}.bak-{secs}"));
        std::fs::copy(path, &b).map_err(|e| format!("sauvegarde impossible : {e}"))?;
        backup = Some(b);
        prune_backups(&dir, &name, keep);
    }
    let tmp = dir.join(format!("{name}.tmp"));
    std::fs::write(&tmp, xml).map_err(|e| format!("écriture impossible : {e}"))?;
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("écriture impossible : {e}")
    })?;
    Ok(backup)
}

// [ICE-20]
pub fn backups(path: &Path) -> Vec<PathBuf> {
    let (Some(dir), Some(name)) = (path.parent(), path.file_name()) else {
        return Vec::new();
    };
    let prefix = format!("{}.bak-", name.to_string_lossy());
    let mut list: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .map(|e| e.path())
                .filter(|p| {
                    p.file_name()
                        .is_some_and(|n| n.to_string_lossy().starts_with(&prefix))
                })
                .collect()
        })
        .unwrap_or_default();
    list.sort();
    list
}

fn prune_backups(dir: &Path, name: &str, keep: usize) {
    let list = backups(&dir.join(name));
    if list.len() > keep {
        for old in &list[..list.len() - keep] {
            let _ = std::fs::remove_file(old);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"<icecast>
    <!-- commentaire conservé -->
    <location>Earth</location>
    <admin>icemaster@localhost</admin>
    <limits>
        <clients>100</clients>
        <sources>2</sources>
        <burst-on-connect>1</burst-on-connect>
        <burst-size>65535</burst-size>
    </limits>
    <authentication>
        <source-password>hackme</source-password>
        <relay-password>hackme</relay-password>
        <admin-user>admin</admin-user>
        <admin-password>hackme</admin-password>
    </authentication>
    <hostname>localhost</hostname>
    <listen-socket>
        <port>8000</port>
    </listen-socket>
    <mount type="normal">
        <mount-name>/live</mount-name>
        <max-listeners>-1</max-listeners>
        <authentication type="htpasswd">
            <option name="filename" value="myauth"/>
        </authentication>
    </mount>
    <fileserve>1</fileserve>
    <paths>
        <logdir>./log</logdir>
        <webroot>./web</webroot>
        <adminroot>./admin</adminroot>
        <alias source="/" destination="/status.xsl"/>
    </paths>
    <logging>
        <accesslog>access.log</accesslog>
        <errorlog>error.log</errorlog>
        <loglevel>3</loglevel>
    </logging>
    <security><chroot>0</chroot></security>
</icecast>"#;

    #[test]
    fn reads_the_default_layout() {
        let cfg = IceDocument::parse(SAMPLE).unwrap().read();
        assert_eq!(cfg.location, "Earth");
        assert_eq!(cfg.auth.admin_password, "hackme");
        assert_eq!(cfg.sockets.len(), 1);
        assert_eq!(cfg.sockets[0].port, 8000);
        assert_eq!(cfg.mounts[0].mount_name, "/live");
        assert_eq!(cfg.mounts[0].max_listeners, 0);
        assert_eq!(cfg.mounts[0].public, None);
        assert_eq!(cfg.limits.queue_size, 524_288);
        assert_eq!(cfg.paths.logdir, "./log");
        let w = cfg.validate().unwrap();
        assert_eq!(w.len(), 3);
    }

    #[test]
    fn writing_keeps_unknown_parts() {
        let mut doc = IceDocument::parse(SAMPLE).unwrap();
        let mut cfg = doc.read();
        cfg.auth.admin_password = "nouveau".into();
        cfg.mounts[0].max_listeners = 50;
        cfg.mounts[0].public = Some(false);
        cfg.mounts.push(Mount {
            mount_name: "/autodj".into(),
            ..Default::default()
        });
        cfg.mounts[0].fallback_mount = "/autodj".into();
        cfg.mounts[0].fallback_override = true;
        cfg.sockets.push(ListenSocket {
            port: 8443,
            ssl: true,
            ..Default::default()
        });
        doc.apply(&cfg);
        let xml = doc.to_xml();
        assert!(xml.contains("commentaire conservé"));
        assert!(xml.contains("<burst-on-connect>1</burst-on-connect>"));
        assert!(xml.contains(r#"<alias source="/" destination="/status.xsl" />"#));
        assert!(xml.contains(r#"<authentication type="htpasswd">"#));
        assert!(xml.contains("<security>"));
        let again = IceDocument::parse(&xml).unwrap().read();
        assert_eq!(again.auth.admin_password, "nouveau");
        assert_eq!(again.mounts.len(), 2);
        assert_eq!(again.mounts[0].max_listeners, 50);
        assert_eq!(again.mounts[0].public, Some(false));
        assert!(again.mounts[0].fallback_override);
        assert_eq!(again.sockets[1].port, 8443);
        assert!(again.sockets[1].ssl);
        assert!(xml.find("/autodj").unwrap() > xml.find("/live").unwrap());
        assert!(xml.find("<mount ").unwrap() < xml.find("<fileserve>").unwrap());
    }

    const SAMPLE_25: &str = r#"<icecast>
    <location>Earth</location>
    <admin>admin@localhost</admin>
    <hostname>localhost</hostname>
    <limits><clients>100</clients><sources>2</sources></limits>
    <authentication>
        <source-password>hackme</source-password>
        <relay-password>hackmelog</relay-password>
        <admin-user>admin</admin-user>
        <admin-password>hackmeadmin</admin-password>
    </authentication>
    <listen-socket>
        <port>8000</port>
        <bind-address>0.0.0.0</bind-address>
    </listen-socket>
    <http-headers>
        <header type="cors" name="Access-Control-Allow-Origin" />
    </http-headers>
    <paths>
        <logdir>./log</logdir>
        <webroot>./web</webroot>
        <adminroot>./admin</adminroot>
        <reportxmldb>./report-db.xml</reportxmldb>
    </paths>
    <logging>
        <accesslog>access.log</accesslog>
        <errorlog>error.log</errorlog>
        <loglevel>information</loglevel>
        <logsize>10000</logsize>
    </logging>
    <security>
        <tls-context>
        </tls-context>
    </security>
</icecast>"#;

    #[test]
    fn keeps_the_icecast_25_layout() {
        let mut doc = IceDocument::parse(SAMPLE_25).unwrap();
        let mut cfg = doc.read();
        assert_eq!(cfg.logging.loglevel, 3);
        assert!(!cfg.sockets[0].ssl);
        doc.apply(&cfg);
        let xml = doc.to_xml();
        assert!(xml.contains("<loglevel>information</loglevel>"));
        assert!(!xml.contains("logarchive"));
        assert!(!xml.contains("<ssl>"));
        assert!(xml.contains("<reportxmldb>./report-db.xml</reportxmldb>"));
        assert!(xml.contains(r#"<header type="cors" name="Access-Control-Allow-Origin" />"#));

        cfg.logging.loglevel = 4;
        cfg.sockets[0].ssl = true;
        cfg.paths.ssl_certificate = "./icecast.pem".into();
        doc.apply(&cfg);
        let xml = doc.to_xml();
        assert!(xml.contains("<loglevel>debug</loglevel>"));
        assert!(xml.contains("<tls>auto_no_plain</tls>"));
        assert!(!xml.contains("<ssl>"));
        assert!(!xml.contains("<ssl-certificate>"));
        assert!(xml.contains("<tls-certificate>./icecast.pem</tls-certificate>"));
        let back = IceDocument::parse(&xml).unwrap().read();
        assert!(back.sockets[0].ssl);
        assert_eq!(back.paths.ssl_certificate, "./icecast.pem");
        assert_eq!(back.logging.loglevel, 4);

        let mut legacy = IceDocument::parse(SAMPLE).unwrap();
        let mut old = legacy.read();
        old.sockets[0].ssl = true;
        legacy.apply(&old);
        let xml = legacy.to_xml();
        assert!(xml.contains("<ssl>1</ssl>"));
        assert!(xml.contains("<loglevel>3</loglevel>"));
    }

    #[test]
    fn removed_mounts_disappear() {
        let mut doc = IceDocument::parse(SAMPLE).unwrap();
        let mut cfg = doc.read();
        cfg.mounts.clear();
        doc.apply(&cfg);
        assert!(!doc.to_xml().contains("<mount"));
    }

    #[test]
    fn validation_catches_mistakes() {
        let base = IceDocument::starter().read();
        assert!(base.validate().unwrap().is_empty());
        let mut c = base.clone();
        c.sockets.push(ListenSocket {
            port: 8000,
            ..Default::default()
        });
        assert!(c.validate().is_err());
        let mut c = base.clone();
        c.sockets[0].shoutcast_mount = "/live".into();
        c.sockets.push(ListenSocket {
            port: 8001,
            ..Default::default()
        });
        assert!(c.validate().is_err());
        let mut c = base.clone();
        c.mounts.push(Mount {
            mount_name: "live".into(),
            ..Default::default()
        });
        assert!(c.validate().is_err());
        let mut c = base.clone();
        c.mounts.push(Mount {
            mount_name: "/a".into(),
            fallback_mount: "/a".into(),
            ..Default::default()
        });
        assert!(c.validate().is_err());
        let mut c = base;
        c.logging.loglevel = 9;
        assert!(c.validate().is_err());
    }

    #[test]
    fn starter_has_strong_passwords() {
        let cfg = IceDocument::starter().read();
        assert_eq!(cfg.auth.admin_user, "admin");
        assert_eq!(cfg.auth.source_password.len(), 16);
        assert_ne!(cfg.auth.source_password, cfg.auth.admin_password);
        assert_eq!(cfg.first_port(), Some(8000));
    }

    #[test]
    fn rebase_makes_paths_absolute() {
        let mut cfg = IceDocument::parse(SAMPLE).unwrap().read();
        let base = std::env::temp_dir().join("ice-base");
        let logs = std::env::temp_dir().join("ice-logs");
        cfg.rebase(&base, &logs);
        assert_eq!(PathBuf::from(&cfg.paths.webroot), base.join("web"));
        assert_eq!(PathBuf::from(&cfg.paths.logdir), logs);
        assert_eq!(
            cfg.log_file(Path::new("/ailleurs"), "error"),
            Some(logs.join("error.log"))
        );
    }

    #[test]
    fn writes_with_rotating_backups() {
        let dir = std::env::temp_dir().join(format!("ice-write-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("icecast.xml");
        assert!(write_file(&path, "<icecast/>", 2).unwrap().is_none());
        for i in 0..4 {
            std::fs::write(dir.join(format!("icecast.xml.bak-{}", 1000 + i)), "x").unwrap();
        }
        assert!(write_file(&path, SAMPLE, 2).unwrap().is_some());
        assert!(backups(&path).len() <= 3);
        assert!(is_writable(&path));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), SAMPLE);
        let _ = std::fs::remove_dir_all(dir);
    }
}
