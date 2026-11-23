use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use blast_cast::{OutputConfig, ServerKind};
use blast_ice::{
    config as icfg, AdminAction, Endpoint, IceConfig, IceDocument, IceError, Install, Listener,
    Process, ServerStats,
};
use serde::Serialize;
use tauri::Manager;

use crate::api::App;
use crate::config::IceServerProfile;
use crate::journal;
use crate::secrets::{ice_key, output_key};

// [ICM-01]
const BACKUPS_KEPT: usize = 10;

// [ICM-02]
pub struct IceManager {
    running: Mutex<HashMap<String, Process>>,
    data_dir: PathBuf,
    config_dir: PathBuf,
}

// [ICM-03]
fn safe_id(id: &str) -> Result<&str, String> {
    if id.is_empty()
        || id.len() > 64
        || !id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err("Identifiant de serveur invalide.".into());
    }
    Ok(id)
}

// [ICM-04]
fn workdir(p: &IceServerProfile) -> Option<PathBuf> {
    if let Some(d) = p.install_dir.as_ref().filter(|d| d.is_dir()) {
        return Some(d.clone());
    }
    let parent = p.exe.as_ref()?.parent()?;
    let is_bin = parent
        .file_name()
        .is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case("bin"));
    Some(if is_bin {
        parent.parent().unwrap_or(parent).to_path_buf()
    } else {
        parent.to_path_buf()
    })
}

impl IceManager {
    pub fn new(data_dir: PathBuf, config_dir: PathBuf) -> Self {
        Self {
            running: Mutex::new(HashMap::new()),
            data_dir,
            config_dir,
        }
    }

    fn dir(&self, id: &str) -> PathBuf {
        self.data_dir.join("icecast").join(id)
    }

    fn pid_file(&self, id: &str) -> PathBuf {
        self.dir(id).join("icecast.pid")
    }

    fn console(&self, id: &str) -> PathBuf {
        self.dir(id).join("console.log")
    }

    pub fn managed_config(&self, id: &str) -> PathBuf {
        self.config_dir.join("icecast").join(id).join("icecast.xml")
    }

    pub fn managed_logs(&self, id: &str) -> PathBuf {
        self.dir(id).join("logs")
    }

    // [ICM-05]
    pub fn state(&self, id: &str) -> (bool, bool) {
        let Ok(mut map) = self.running.lock() else {
            return (false, false);
        };
        if let Some(p) = map.get_mut(id) {
            if p.alive() {
                return (true, p.owned());
            }
            map.remove(id);
            let _ = std::fs::remove_file(self.pid_file(id));
        }
        let pid = std::fs::read_to_string(self.pid_file(id))
            .ok()
            .and_then(|t| t.trim().parse::<u32>().ok());
        if let Some(p) = pid.and_then(Process::adopt) {
            map.insert(id.to_string(), p);
            return (true, false);
        }
        (false, false)
    }

    // [ICM-06]
    pub fn start(&self, p: &IceServerProfile) -> Result<String, String> {
        if p.remote {
            return Err(
                "Le lancement d'un serveur distant demandera une connexion SSH (prochaine étape)."
                    .into(),
            );
        }
        if self.state(&p.id).0 {
            return Ok("Icecast tourne déjà.".into());
        }
        let exe = p
            .exe
            .clone()
            .filter(|e| e.is_file())
            .ok_or("Exécutable Icecast introuvable : vérifie le dossier d'installation.")?;
        let config = p
            .config_path
            .clone()
            .filter(|c| c.is_file())
            .ok_or("Fichier de configuration introuvable.")?;
        let cfg = IceDocument::load(&config)?.read();
        cfg.validate()?;
        let wd = workdir(p).ok_or("Dossier de travail introuvable.")?;
        if let Some(log) = cfg.log_file(&wd, "error") {
            if let Some(dir) = log.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            if !icfg::is_writable(&log) {
                return Err(format!(
                    "Icecast ne pourra pas écrire son journal dans {} (dossier protégé). Onglet CONFIGURATION : « Copier la configuration dans BLAST ».",
                    log.parent().unwrap_or(&log).display()
                ));
            }
        }
        let proc = Process::start(&exe, &config, &wd, &self.console(&p.id))?;
        let _ = std::fs::write(self.pid_file(&p.id), proc.pid.to_string());
        journal::info(format!("icecast {} : lancé (pid {})", p.name, proc.pid));
        let pid = proc.pid;
        if let Ok(mut map) = self.running.lock() {
            map.insert(p.id.clone(), proc);
        }
        Ok(format!("Icecast lancé (processus {pid})."))
    }

    // [ICM-07]
    pub fn stop(&self, id: &str) -> Result<String, String> {
        let _ = self.state(id);
        let proc = self.running.lock().ok().and_then(|mut m| m.remove(id));
        let _ = std::fs::remove_file(self.pid_file(id));
        match proc {
            Some(mut p) => {
                p.stop()?;
                journal::info(format!("icecast {id} : arrêté"));
                Ok("Icecast arrêté.".into())
            }
            None => Err("BLAST ne trouve pas de processus Icecast lancé par lui. S'il tourne comme service Windows, arrête-le depuis les Services.".into()),
        }
    }

    // [ICM-08]
    pub fn stop_on_exit(&self, profiles: &[IceServerProfile]) {
        for p in profiles.iter().filter(|p| p.stop_on_exit && !p.remote) {
            if self.state(&p.id) == (true, true) {
                let _ = self.stop(&p.id);
            }
        }
    }
}

// [ICM-09]
#[derive(Serialize)]
pub struct ServerView {
    #[serde(flatten)]
    profile: IceServerProfile,
    has_password: bool,
    managed: bool,
}

// [ICM-10]
#[derive(Serialize)]
pub struct IceView {
    servers: Vec<ServerView>,
    detected: Vec<Install>,
    secret_store: String,
    warning: Option<String>,
}

// [ICM-11]
#[derive(Serialize, Default)]
pub struct StatusView {
    running: bool,
    owned: bool,
    reachable: bool,
    server_id: Option<String>,
    stats: Option<ServerStats>,
    error: Option<String>,
    checked_at: u64,
}

// [ICM-12]
#[derive(Serialize)]
pub struct ConfigView {
    config: IceConfig,
    path: String,
    writable: bool,
    managed: bool,
    backups: usize,
    warnings: Vec<String>,
    message: Option<String>,
}

impl App {
    fn ice_profile(&self, id: &str) -> Result<IceServerProfile, String> {
        safe_id(id)?;
        self.config
            .lock()
            .map_err(|e| e.to_string())?
            .icecast
            .servers
            .iter()
            .find(|s| s.id == id)
            .cloned()
            .ok_or_else(|| "Serveur Icecast inconnu.".into())
    }

    fn ice_update(&self, id: &str, edit: impl FnOnce(&mut IceServerProfile)) -> Result<(), String> {
        let id = id.to_string();
        self.commit(move |cfg| {
            if let Some(s) = cfg.icecast.servers.iter_mut().find(|s| s.id == id) {
                edit(s);
            }
        })
    }

    fn ice_endpoint(&self, p: &IceServerProfile) -> Endpoint {
        let stored = self
            .secrets
            .get(&ice_key(&p.id))
            .filter(|pw| !pw.is_empty());
        let from_file = || {
            p.config_path
                .as_ref()
                .filter(|_| !p.remote)
                .and_then(|c| IceDocument::load(c).ok())
                .map(|d| d.read().auth)
                .filter(|a| a.admin_user == p.admin_user)
                .map(|a| a.admin_password)
        };
        Endpoint {
            host: p.host.clone(),
            port: p.port,
            https: p.https,
            user: p.admin_user.clone(),
            password: stored.or_else(from_file).unwrap_or_default(),
            user_agent: format!("BLAST-Broadcaster/{}", crate::update::CURRENT),
        }
    }

    // [ICM-13]
    fn ice_view(&self, warning: Option<String>) -> IceView {
        let servers = self
            .config
            .lock()
            .map(|c| c.icecast.servers.clone())
            .unwrap_or_default();
        IceView {
            servers: servers
                .into_iter()
                .map(|p| ServerView {
                    has_password: self.secrets.has(&ice_key(&p.id)),
                    managed: p
                        .config_path
                        .as_ref()
                        .is_some_and(|c| *c == self.ice.managed_config(&p.id)),
                    profile: p,
                })
                .collect(),
            detected: blast_ice::detect(),
            secret_store: self.secrets.store_label(),
            warning,
        }
    }

    // [ICM-14]
    fn ice_config_view(
        &self,
        p: &IceServerProfile,
        warnings: Vec<String>,
        message: Option<String>,
    ) -> Result<ConfigView, String> {
        if p.remote {
            return Err("La configuration d'un serveur distant se modifiera par SSH (prochaine étape). Le tableau de bord, les auditeurs et les actions admin fonctionnent déjà.".into());
        }
        let path = p
            .config_path
            .clone()
            .ok_or("Aucun fichier de configuration : choisis-en un ou crée-en un nouveau.")?;
        let doc = IceDocument::load(&path)?;
        Ok(ConfigView {
            config: doc.read(),
            writable: icfg::is_writable(&path),
            managed: path == self.ice.managed_config(&p.id),
            backups: icfg::backups(&path).len(),
            path: path.display().to_string(),
            warnings,
            message,
        })
    }

    fn ice_sync_credentials(&self, p: &IceServerProfile, cfg: &IceConfig) -> Option<String> {
        let key = ice_key(&p.id);
        let warn = if self.secrets.get(&key).as_deref() == Some(cfg.auth.admin_password.as_str()) {
            None
        } else {
            self.store_password(&key, Some(cfg.auth.admin_password.clone()))
        };
        let user = cfg.auth.admin_user.clone();
        let port = cfg.first_port();
        let _ = self.ice_update(&p.id, move |s| {
            s.admin_user = user;
            if let Some(port) = port {
                s.port = port;
            }
        });
        warn
    }
}

async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| e.to_string())?
}

// [ICM-15]
#[tauri::command]
pub fn ice_list(state: tauri::State<'_, App>) -> IceView {
    state.ice_view(None)
}

// [ICM-16]
#[tauri::command]
pub fn ice_detect(dir: Option<PathBuf>) -> Vec<Install> {
    match dir {
        Some(d) => blast_ice::find_in(&d).into_iter().collect(),
        None => blast_ice::detect(),
    }
}

// [ICM-17]
#[tauri::command]
pub fn ice_save_server(
    state: tauri::State<'_, App>,
    server: IceServerProfile,
    password: Option<String>,
) -> Result<IceView, String> {
    let mut server = server;
    server.name = server.name.trim().to_string();
    server.host = server.host.trim().to_string();
    if server.name.is_empty() {
        return Err("Donne un nom à ce serveur.".into());
    }
    if server.host.is_empty() || server.port == 0 {
        return Err("Indique l'adresse et le port du serveur.".into());
    }
    if server.id.is_empty() {
        server.id = format!("ice-{}", crate::controller::now_ms());
    }
    safe_id(&server.id)?;
    let mut notes = Vec::new();
    let mut from_config = None;
    if server.remote {
        server.install_dir = None;
        server.exe = None;
        server.config_path = None;
        server.autostart = false;
        server.stop_on_exit = false;
        if !server.https && !matches!(server.host.as_str(), "localhost" | "127.0.0.1" | "::1") {
            notes.push("Connexion en HTTP : le mot de passe admin circulera en clair sur le réseau. Préfère HTTPS (Icecast derrière un proxy TLS) ou un tunnel SSH.".to_string());
        }
    } else {
        if server.exe.as_ref().is_none_or(|e| !e.is_file()) {
            let dir = server
                .install_dir
                .clone()
                .ok_or("Indique le dossier d'installation d'Icecast.")?;
            let found = blast_ice::find_in(&dir)
                .ok_or_else(|| format!("Aucun exécutable Icecast dans {}", dir.display()))?;
            server.exe = Some(found.exe);
            if server.config_path.is_none() {
                server.config_path = found.config;
            }
        }
        if let Some(c) = server.config_path.as_ref() {
            from_config = Some(IceDocument::load(c)?.read());
        }
    }
    let mut password = password.filter(|p| !p.is_empty());
    if let Some(cfg) = &from_config {
        if let Some(port) = cfg.first_port() {
            server.port = port;
        }
        if !cfg.auth.admin_user.is_empty() {
            server.admin_user = cfg.auth.admin_user.clone();
        }
        if password.is_none() && !cfg.auth.admin_password.is_empty() {
            password = Some(cfg.auth.admin_password.clone());
        }
    }
    if let Some(w) = state.store_password(&ice_key(&server.id), password) {
        notes.push(w);
    }
    let saved = server.clone();
    state.commit(
        move |cfg| match cfg.icecast.servers.iter_mut().find(|s| s.id == saved.id) {
            Some(s) => *s = saved,
            None => cfg.icecast.servers.push(saved),
        },
    )?;
    Ok(state.ice_view((!notes.is_empty()).then(|| notes.join(" "))))
}

// [ICM-18]
#[tauri::command]
pub fn ice_delete_server(state: tauri::State<'_, App>, id: String) -> Result<IceView, String> {
    safe_id(&id)?;
    if state.ice.state(&id) == (true, true) {
        let _ = state.ice.stop(&id);
    }
    state.secrets.delete(&ice_key(&id));
    state.commit(|cfg| cfg.icecast.servers.retain(|s| s.id != id))?;
    Ok(state.ice_view(None))
}

// [ICM-19]
#[tauri::command]
pub async fn ice_status(app: tauri::AppHandle, id: String) -> Result<StatusView, String> {
    let state = app.state::<App>();
    let p = state.ice_profile(&id)?;
    let ep = state.ice_endpoint(&p);
    let has_password = !ep.password.is_empty();
    let app2 = app.clone();
    blocking(move || {
        let state = app2.state::<App>();
        let (running, owned) = if p.remote {
            (false, false)
        } else {
            state.ice.state(&p.id)
        };
        let mut v = StatusView {
            running,
            owned,
            checked_at: crate::controller::now_ms(),
            ..Default::default()
        };
        let stats = if has_password {
            ep.stats()
        } else {
            Err(IceError::Unauthorized)
        };
        match stats {
            Ok(s) => {
                v.reachable = true;
                v.server_id = Some(s.server_id.clone());
                v.stats = Some(s);
            }
            Err(IceError::Unreachable(e)) => {
                v.error = Some(if running {
                    format!(
                        "Icecast démarre ou n'écoute pas sur {}:{} ({e})",
                        p.host, p.port
                    )
                } else {
                    format!("Serveur injoignable sur {}:{}", p.host, p.port)
                });
            }
            Err(e) => match ep.probe() {
                Ok(id) => {
                    v.reachable = true;
                    v.server_id = Some(id);
                    v.error = Some(if has_password {
                        format!("Tableau de bord indisponible : {e}")
                    } else {
                        "Saisis le mot de passe admin dans le profil pour voir les statistiques."
                            .into()
                    });
                }
                Err(e2) => v.error = Some(e2.to_string()),
            },
        }
        if p.remote {
            v.running = v.reachable;
        }
        Ok(v)
    })
    .await
}

// [ICM-20]
#[tauri::command]
pub async fn ice_power(
    app: tauri::AppHandle,
    id: String,
    action: String,
) -> Result<String, String> {
    let p = app.state::<App>().ice_profile(&id)?;
    blocking(move || {
        let state = app.state::<App>();
        match action.as_str() {
            "start" => state.ice.start(&p),
            "stop" => state.ice.stop(&p.id),
            "restart" => {
                if state.ice.state(&p.id).0 {
                    state.ice.stop(&p.id)?;
                    std::thread::sleep(std::time::Duration::from_millis(600));
                }
                state.ice.start(&p).map(|_| "Icecast redémarré.".into())
            }
            _ => Err("Action inconnue.".into()),
        }
    })
    .await
}

// [ICM-21]
#[tauri::command]
pub fn ice_get_config(state: tauri::State<'_, App>, id: String) -> Result<ConfigView, String> {
    let p = state.ice_profile(&id)?;
    state.ice_config_view(&p, Vec::new(), None)
}

// [ICM-22]
#[tauri::command]
pub async fn ice_save_config(
    app: tauri::AppHandle,
    id: String,
    config: IceConfig,
    restart: bool,
) -> Result<ConfigView, String> {
    blocking(move || {
        let state = app.state::<App>();
        let p = state.ice_profile(&id)?;
        let path = p
            .config_path
            .clone()
            .ok_or("Aucun fichier de configuration.")?;
        let mut warnings = config.validate()?;
        if !icfg::is_writable(&path) {
            return Err(format!(
                "Windows refuse l'écriture dans {} (dossier protégé). Utilise « Copier la configuration dans BLAST », ou lance BLAST en administrateur.",
                path.display()
            ));
        }
        let mut doc = IceDocument::load(&path)?;
        doc.apply(&config);
        let xml = doc.to_xml();
        IceDocument::parse(&xml)?;
        icfg::write_file(&path, &xml, BACKUPS_KEPT)?;
        journal::info(format!("icecast {} : configuration enregistrée", p.name));
        if let Some(w) = state.ice_sync_credentials(&p, &config) {
            warnings.push(w);
        }
        let p = state.ice_profile(&id)?;
        let (running, _) = state.ice.state(&p.id);
        let message = if restart && running {
            state.ice.stop(&p.id)?;
            std::thread::sleep(std::time::Duration::from_millis(600));
            state.ice.start(&p)?;
            "Configuration enregistrée, Icecast redémarré.".to_string()
        } else if restart {
            state.ice.start(&p)?;
            "Configuration enregistrée, Icecast lancé.".to_string()
        } else if running {
            "Configuration enregistrée. Elle s'appliquera au prochain redémarrage d'Icecast.".to_string()
        } else {
            "Configuration enregistrée.".to_string()
        };
        state.ice_config_view(&p, warnings, Some(message))
    })
    .await
}

// [ICM-23]
#[tauri::command]
pub fn ice_adopt_config(state: tauri::State<'_, App>, id: String) -> Result<ConfigView, String> {
    let p = state.ice_profile(&id)?;
    let src = p
        .config_path
        .clone()
        .ok_or("Aucun fichier de configuration à copier.")?;
    let target = state.ice.managed_config(&p.id);
    if src == target {
        return state.ice_config_view(&p, Vec::new(), None);
    }
    let wd = workdir(&p).ok_or("Dossier d'installation introuvable.")?;
    let logs = state.ice.managed_logs(&p.id);
    std::fs::create_dir_all(&logs).map_err(|e| e.to_string())?;
    let mut doc = IceDocument::load(&src)?;
    let mut cfg = doc.read();
    cfg.rebase(&wd, &logs);
    doc.apply(&cfg);
    icfg::write_file(&target, &doc.to_xml(), BACKUPS_KEPT)?;
    let t = target.clone();
    state.ice_update(&p.id, move |s| s.config_path = Some(t))?;
    let p = state.ice_profile(&id)?;
    state.ice_config_view(
        &p,
        Vec::new(),
        Some(format!(
            "Configuration copiée dans {} : BLAST peut maintenant la modifier. L'original n'est pas touché.",
            target.display()
        )),
    )
}

// [ICM-24]
#[tauri::command]
pub fn ice_new_config(state: tauri::State<'_, App>, id: String) -> Result<ConfigView, String> {
    let p = state.ice_profile(&id)?;
    if p.remote {
        return Err("Disponible pour un serveur local seulement.".into());
    }
    let target = state.ice.managed_config(&p.id);
    if target.exists() {
        return Err(format!(
            "Une configuration créée par BLAST existe déjà : {}",
            target.display()
        ));
    }
    let wd = workdir(&p).ok_or("Indique d'abord le dossier d'installation d'Icecast.")?;
    let install = p.install_dir.as_deref().and_then(blast_ice::find_in);
    let logs = state.ice.managed_logs(&p.id);
    std::fs::create_dir_all(&logs).map_err(|e| e.to_string())?;
    let mut doc = IceDocument::starter();
    let mut cfg = doc.read();
    cfg.rebase(&wd, &logs);
    let linux_share = |sub: &str| {
        ["/usr/share/icecast2", "/usr/share/icecast"]
            .iter()
            .map(|d| Path::new(d).join(sub))
            .find(|d| d.is_dir())
    };
    if let Some(w) = install
        .as_ref()
        .and_then(|i| i.web.clone())
        .or_else(|| linux_share("web"))
    {
        cfg.paths.webroot = w.display().to_string();
    }
    if let Some(a) = install
        .as_ref()
        .and_then(|i| i.admin.clone())
        .or_else(|| linux_share("admin"))
    {
        cfg.paths.adminroot = a.display().to_string();
    }
    doc.apply(&cfg);
    icfg::write_file(&target, &doc.to_xml(), BACKUPS_KEPT)?;
    let t = target.clone();
    state.ice_update(&p.id, move |s| s.config_path = Some(t))?;
    let p = state.ice_profile(&id)?;
    let warn = state.ice_sync_credentials(&p, &cfg);
    let p = state.ice_profile(&id)?;
    state.ice_config_view(
        &p,
        warn.into_iter().collect(),
        Some("Nouvelle configuration créée avec des mots de passe aléatoires.".into()),
    )
}

// [ICM-25]
#[tauri::command]
pub async fn ice_listeners(
    app: tauri::AppHandle,
    id: String,
    mount: String,
) -> Result<Vec<Listener>, String> {
    let state = app.state::<App>();
    let ep = state.ice_endpoint(&state.ice_profile(&id)?);
    blocking(move || ep.listeners(&mount).map_err(|e| e.to_string())).await
}

// [ICM-26]
#[tauri::command]
pub async fn ice_admin(
    app: tauri::AppHandle,
    id: String,
    action: AdminAction,
) -> Result<String, String> {
    let state = app.state::<App>();
    let p = state.ice_profile(&id)?;
    let ep = state.ice_endpoint(&p);
    let name = p.name.clone();
    blocking(move || {
        let r = ep.run(&action).map_err(|e| e.to_string());
        if r.is_ok() {
            journal::info(format!("icecast {name} : action admin {action:?}"));
        }
        r
    })
    .await
}

// [ICM-27]
#[tauri::command]
pub fn ice_log(
    state: tauri::State<'_, App>,
    id: String,
    which: String,
    lines: Option<usize>,
) -> Result<String, String> {
    let p = state.ice_profile(&id)?;
    let lines = lines.unwrap_or(300).clamp(10, 5000);
    let path = if which == "console" {
        state.ice.console(&p.id)
    } else {
        let cfg_path = p
            .config_path
            .clone()
            .ok_or("Aucun fichier de configuration.")?;
        let cfg = IceDocument::load(&cfg_path)?.read();
        let wd = workdir(&p).unwrap_or_default();
        cfg.log_file(&wd, &which)
            .ok_or("Ce journal n'est pas activé dans la configuration.")?
    };
    if !path.is_file() {
        return Ok(format!(
            "(journal vide ou pas encore créé : {})",
            path.display()
        ));
    }
    blast_ice::process::tail(&path, lines)
}

// [ICM-28]
#[tauri::command]
pub fn ice_link_output(
    state: tauri::State<'_, App>,
    id: String,
    mount: String,
) -> Result<String, String> {
    let p = state.ice_profile(&id)?;
    let cfg = p
        .config_path
        .as_ref()
        .and_then(|c| IceDocument::load(c).ok())
        .map(|d| d.read());
    let m = cfg
        .as_ref()
        .and_then(|c| c.mounts.iter().find(|m| m.mount_name == mount).cloned());
    let password = m
        .as_ref()
        .map(|m| m.password.clone())
        .filter(|pw| !pw.is_empty())
        .or_else(|| cfg.as_ref().map(|c| c.auth.source_password.clone()))
        .filter(|pw| !pw.is_empty());
    let exists = state
        .config
        .lock()
        .map_err(|e| e.to_string())?
        .outputs
        .iter()
        .any(|o| {
            o.kind == ServerKind::Icecast
                && o.port == p.port
                && o.mount_path() == mount
                && o.host == p.host
        });
    if exists {
        return Err(format!(
            "Une sortie vers {mount} existe déjà dans Réglages → Diffusion."
        ));
    }
    let out = OutputConfig {
        id: format!("out-{}", crate::controller::now_ms()),
        name: format!("{} {mount}", p.name),
        kind: ServerKind::Icecast,
        host: p.host.clone(),
        port: p.port,
        mount: mount.clone(),
        user: m
            .as_ref()
            .map(|m| m.username.clone())
            .filter(|u| !u.is_empty())
            .unwrap_or_else(|| "source".into()),
        station: m
            .as_ref()
            .map(|m| m.stream_name.clone())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "BLAST Broadcaster".into()),
        genre: m.as_ref().map(|m| m.genre.clone()).unwrap_or_default(),
        description: m
            .as_ref()
            .map(|m| m.stream_description.clone())
            .unwrap_or_default(),
        url: m.as_ref().map(|m| m.stream_url.clone()).unwrap_or_default(),
        public: m.as_ref().and_then(|m| m.public).unwrap_or(false),
        ..Default::default()
    };
    out.validate()?;
    let has_pw = password.is_some();
    let warn = state.store_password(&output_key(&out.id), password);
    state.commit(move |cfg| cfg.outputs.push(out))?;
    let mut msg = format!(
        "Sortie « {} {mount} » ajoutée dans Réglages → Diffusion.",
        p.name
    );
    if !has_pw {
        msg.push_str(" Saisis son mot de passe source.");
    }
    if let Some(w) = warn {
        msg.push(' ');
        msg.push_str(&w);
    }
    Ok(msg)
}

// [ICM-30]
#[tauri::command]
pub fn ice_open_web(state: tauri::State<'_, App>, id: String) -> Result<(), String> {
    let p = state.ice_profile(&id)?;
    let host = if matches!(p.host.as_str(), "0.0.0.0" | "::") {
        "localhost".to_string()
    } else {
        p.host.clone()
    };
    let url = Endpoint {
        host,
        ..state.ice_endpoint(&p)
    }
    .base();
    open::that_detached(format!("{url}/")).map_err(|e| e.to_string())
}

// [ICM-29]
pub fn autostart(app: &tauri::AppHandle) {
    let state = app.state::<App>();
    let servers: Vec<IceServerProfile> = state
        .config
        .lock()
        .map(|c| c.icecast.servers.clone())
        .unwrap_or_default();
    let todo: Vec<IceServerProfile> = servers
        .into_iter()
        .filter(|s| s.autostart && !s.remote)
        .collect();
    if todo.is_empty() {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        let state = app.state::<App>();
        for p in todo {
            if let Err(e) = state.ice.start(&p) {
                journal::warn(format!(
                    "icecast {} : lancement automatique impossible : {e}",
                    p.name
                ));
            }
        }
    });
}
