use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use blast_aura::AuraSettings;
use blast_cast::OutputConfig;
use crossbeam_channel::Sender;
use serde::Serialize;
use tauri::Manager;

use crate::config::{
    self, AppConfig, AppearanceSettings, AudioSettings, AutomationSettings, CategorySettings,
    JingleSettings, PadConfig, SoundboardSettings, VoiceSettings, PAD_COUNT,
};
use crate::controller::UiCommand;
use crate::secrets::{output_key, Secrets, Stored, AURA_KEY};
use crate::{journal, themes, update};
use blast_library::{HistoryStore, Kind};

// [APP-03]
pub struct App {
    pub tx: Sender<UiCommand>,
    pub config: Mutex<AppConfig>,
    pub config_path: PathBuf,
    pub db_path: PathBuf,
    pub secrets: Arc<Secrets>,
}

// [APP-04]
#[derive(Serialize)]
pub struct OutputView {
    #[serde(flatten)]
    cfg: OutputConfig,
    has_password: bool,
}

// [APP-05]
#[derive(Serialize)]
pub struct SettingsView {
    outputs: Vec<OutputView>,
    aura: AuraSettings,
    aura_has_password: bool,
    audio: AudioSettings,
    automation: AutomationSettings,
    categories: CategorySettings,
    jingles: JingleSettings,
    library_dirs: Vec<PathBuf>,
    rotation: blast_library::Rotation,
    library: Option<blast_library::LibraryStats>,
    db_path: String,
    voice: VoiceSettings,
    soundboard: SoundboardSettings,
    devices: Vec<String>,
    default_device: Option<String>,
    input_devices: Vec<String>,
    default_input: Option<String>,
    voice_presets: Vec<(&'static str, &'static str)>,
    master_presets: Vec<(&'static str, &'static str)>,
    secret_store: String,
    config_path: String,
    warning: Option<String>,
}

// [APP-06]
impl App {
    fn view(&self, warning: Option<String>) -> SettingsView {
        let cfg = self.config.lock().map(|c| c.clone()).unwrap_or_default();
        SettingsView {
            outputs: cfg
                .outputs
                .iter()
                .map(|o| OutputView {
                    has_password: self.secrets.has(&output_key(&o.id)),
                    cfg: o.clone(),
                })
                .collect(),
            aura_has_password: self.secrets.has(AURA_KEY),
            aura: cfg.aura,
            audio: cfg.audio,
            automation: cfg.automation,
            categories: cfg.categories,
            jingles: cfg.jingles,
            library_dirs: cfg.library.dirs,
            rotation: cfg.rotation,
            library: blast_library::SqliteStore::open(&self.db_path)
                .ok()
                .and_then(|l| l.stats().ok()),
            db_path: self.db_path.display().to_string(),
            voice: cfg.voice,
            soundboard: cfg.soundboard,
            devices: blast_core::engine::list_output_devices().unwrap_or_default(),
            default_device: blast_core::engine::default_output_device(),
            input_devices: blast_core::engine::list_input_devices().unwrap_or_default(),
            default_input: blast_core::engine::default_input_device(),
            voice_presets: blast_core::VOICE_PRESETS.to_vec(),
            master_presets: blast_core::MASTER_PRESETS.to_vec(),
            secret_store: self.secrets.store_label(),
            config_path: self.config_path.display().to_string(),
            warning,
        }
    }

    fn commit(&self, edit: impl FnOnce(&mut AppConfig)) -> Result<(), String> {
        let mut cfg = self.config.lock().map_err(|e| e.to_string())?;
        edit(&mut cfg);
        config::save(&self.config_path, &cfg)?;
        let _ = self.tx.send(UiCommand::Config(Box::new(cfg.clone())));
        Ok(())
    }

    fn store_password(&self, key: &str, password: Option<String>) -> Option<String> {
        let pw = password.filter(|p| !p.is_empty())?;
        match self.secrets.set(key, &pw) {
            Stored::Keyring => None,
            Stored::Memory(e) => Some(format!(
                "Mot de passe gardé en mémoire pour cette session seulement : le trousseau du système est indisponible ({e})."
            )),
        }
    }
}

// [APP-07]
#[tauri::command]
pub fn blast(state: tauri::State<'_, App>, cmd: UiCommand) -> Result<(), String> {
    state.tx.send(cmd).map_err(|e| e.to_string())
}

// [APP-08]
#[tauri::command]
pub fn themes(app: tauri::AppHandle) -> Vec<serde_json::Value> {
    let dir = app.path().app_config_dir().ok().map(|d| d.join("themes"));
    themes::list(dir.as_deref())
}

// [APP-09]
#[tauri::command]
pub fn themes_dir(app: tauri::AppHandle) -> Option<String> {
    let dir = app.path().app_config_dir().ok()?.join("themes");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir.to_string_lossy().into_owned())
}

// [APP-10]
#[tauri::command]
pub fn get_settings(state: tauri::State<'_, App>) -> SettingsView {
    state.view(None)
}

// [APP-11]
#[tauri::command]
pub fn save_output(
    state: tauri::State<'_, App>,
    mut output: OutputConfig,
    password: Option<String>,
) -> Result<SettingsView, String> {
    output.validate()?;
    if output.id.is_empty() {
        let n = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        output.id = format!("out-{n}");
    }
    let warning = state.store_password(&output_key(&output.id), password);
    let saved = output.clone();
    state.commit(
        move |cfg| match cfg.outputs.iter_mut().find(|o| o.id == saved.id) {
            Some(o) => *o = saved,
            None => cfg.outputs.push(saved),
        },
    )?;
    Ok(state.view(warning))
}

// [APP-12]
#[tauri::command]
pub fn delete_output(state: tauri::State<'_, App>, id: String) -> Result<SettingsView, String> {
    state.secrets.delete(&output_key(&id));
    state.commit(|cfg| cfg.outputs.retain(|o| o.id != id))?;
    Ok(state.view(None))
}

// [APP-13]
#[tauri::command]
pub async fn test_output(
    state: tauri::State<'_, App>,
    output: OutputConfig,
    password: Option<String>,
) -> Result<String, String> {
    output.validate()?;
    let pw = password
        .filter(|p| !p.is_empty())
        .or_else(|| state.secrets.get(&output_key(&output.id)))
        .ok_or("Saisis le mot de passe de la source.")?;
    tauri::async_runtime::spawn_blocking(move || {
        blast_cast::test_output(&output, &pw, 44_100).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

// [APP-14]
#[tauri::command]
pub fn save_aura(
    state: tauri::State<'_, App>,
    aura: AuraSettings,
    password: Option<String>,
) -> Result<SettingsView, String> {
    if aura.host.trim().is_empty() || aura.port == 0 {
        return Err("Indique l'adresse et le port du serveur WebSocket d'OBS.".into());
    }
    let warning = state.store_password(AURA_KEY, password);
    state.commit(move |cfg| cfg.aura = aura)?;
    Ok(state.view(warning))
}

// [APP-15]
#[tauri::command]
pub async fn test_aura(
    state: tauri::State<'_, App>,
    aura: AuraSettings,
    password: Option<String>,
) -> Result<String, String> {
    let pw = password
        .filter(|p| !p.is_empty())
        .or_else(|| state.secrets.get(AURA_KEY));
    tauri::async_runtime::spawn_blocking(move || {
        blast_aura::test_connection(&aura, pw.as_deref())
            .map(|i| {
                format!(
                    "Connecté à OBS {} (obs-websocket {}){}",
                    i.obs_version,
                    i.websocket_version,
                    if i.platform.is_empty() {
                        String::new()
                    } else {
                        format!(" sur {}", i.platform)
                    }
                )
            })
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

// [APP-16]
#[tauri::command]
pub fn save_audio(
    state: tauri::State<'_, App>,
    device: Option<String>,
) -> Result<SettingsView, String> {
    let device = device.filter(|d| !d.is_empty());
    state.commit(move |cfg| cfg.audio.device = device)?;
    Ok(state.view(Some(
        "La nouvelle sortie audio sera utilisée au prochain démarrage de BLAST.".into(),
    )))
}

// [APP-17]
#[tauri::command]
pub fn save_voice(state: tauri::State<'_, App>, voice: VoiceSettings) -> Result<(), String> {
    state.commit(move |cfg| {
        let VoiceSettings {
            enabled,
            input_device,
            ..
        } = cfg.voice.clone();
        cfg.voice = VoiceSettings {
            enabled,
            input_device,
            ..voice
        };
    })
}

// [APP-18]
#[tauri::command]
pub fn save_input(
    state: tauri::State<'_, App>,
    enabled: bool,
    device: Option<String>,
) -> Result<SettingsView, String> {
    let device = device.filter(|d| !d.is_empty());
    state.commit(move |cfg| {
        cfg.voice.enabled = enabled;
        cfg.voice.input_device = device;
    })?;
    Ok(state.view(Some(
        "Le nouveau micro sera utilisé au prochain démarrage de BLAST.".into(),
    )))
}

// [APP-19]
#[tauri::command]
pub fn save_pad(state: tauri::State<'_, App>, pad: PadConfig) -> Result<(), String> {
    if pad.index >= PAD_COUNT {
        return Err("Pad inexistant.".into());
    }
    if !pad.path.is_file() {
        return Err(format!("Fichier introuvable : {}", pad.path.display()));
    }
    let mut pad = pad;
    if pad.name.trim().is_empty() {
        pad.name = pad
            .path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
    }
    state.commit(move |cfg| {
        cfg.soundboard.pads.retain(|p| p.index != pad.index);
        cfg.soundboard.pads.push(pad);
        cfg.soundboard.pads.sort_by_key(|p| p.index);
    })
}

// [APP-20]
#[tauri::command]
pub fn clear_pad(state: tauri::State<'_, App>, index: u16) -> Result<(), String> {
    state.commit(move |cfg| cfg.soundboard.pads.retain(|p| p.index != index))
}

// [APP-21]
#[tauri::command]
pub fn save_sfx_volume(state: tauri::State<'_, App>, volume_db: f32) -> Result<(), String> {
    state.commit(move |cfg| cfg.soundboard.volume_db = volume_db.clamp(-60.0, 6.0))
}

// [APP-22]
#[tauri::command]
pub fn save_master_preset(state: tauri::State<'_, App>, preset: String) -> Result<(), String> {
    if !blast_core::MASTER_PRESETS
        .iter()
        .any(|(id, _)| *id == preset)
    {
        return Err(format!("Préréglage inconnu : {preset}"));
    }
    state.commit(move |cfg| cfg.audio.master_preset = preset)
}

// [APP-23]
#[tauri::command]
pub fn save_automation(
    state: tauri::State<'_, App>,
    automation: AutomationSettings,
) -> Result<SettingsView, String> {
    if automation.memory_size == 0 || automation.memory_size > 5000 {
        return Err(
            "La mémoire de l'historique doit être comprise entre 1 et 5000 passages.".into(),
        );
    }
    state.commit(move |cfg| cfg.automation = automation)?;
    Ok(state.view(None))
}

// [APP-31]
#[tauri::command]
pub fn save_categories(
    state: tauri::State<'_, App>,
    categories: CategorySettings,
) -> Result<SettingsView, String> {
    let clean = |list: Vec<String>| -> Vec<String> {
        list.into_iter()
            .map(|k| k.trim().to_string())
            .filter(|k| !k.is_empty())
            .collect()
    };
    let categories = CategorySettings {
        jingle_keywords: clean(categories.jingle_keywords),
        pub_keywords: clean(categories.pub_keywords),
    };
    state.commit(move |cfg| cfg.categories = categories)?;
    Ok(state.view(None))
}

// [APP-32]
#[tauri::command]
pub fn save_jingles(
    state: tauri::State<'_, App>,
    dirs: Option<Vec<PathBuf>>,
    volume_db: Option<f32>,
) -> Result<(), String> {
    if let Some(d) = &dirs {
        if let Some(missing) = d.iter().find(|p| !p.is_dir()) {
            return Err(format!("Dossier introuvable : {}", missing.display()));
        }
    }
    state.commit(move |cfg| {
        if let Some(d) = dirs {
            cfg.jingles.dirs = d;
        }
        if let Some(v) = volume_db {
            cfg.jingles.volume_db = v.clamp(-60.0, 6.0);
        }
    })
}

// [APP-24]
#[tauri::command]
pub fn export_history(
    state: tauri::State<'_, App>,
    path: String,
    kind: Option<String>,
    from: Option<i64>,
    to: Option<i64>,
) -> Result<usize, String> {
    let kind = match kind.as_deref() {
        None | Some("") | Some("all") => None,
        Some(k) => Some(Kind::parse(k).ok_or_else(|| format!("Catégorie inconnue : {k}"))?),
    };
    let lib = blast_library::SqliteStore::open(&state.db_path).map_err(|e| e.to_string())?;
    let rows = lib
        .history_between(from.unwrap_or(0), to.unwrap_or(i64::MAX), kind, u32::MAX)
        .map_err(|e| e.to_string())?;
    let csv = blast_library::history_csv(&rows, |t| {
        chrono::DateTime::from_timestamp(t, 0)
            .map(|d| {
                d.with_timezone(&chrono::Local)
                    .format("%Y-%m-%d %H:%M:%S")
                    .to_string()
            })
            .unwrap_or_default()
    });
    let mut path = PathBuf::from(path);
    if path.extension().is_none() {
        path.set_extension("csv");
    }
    std::fs::write(&path, csv).map_err(|e| e.to_string())?;
    Ok(rows.len())
}

// [APP-25]
#[derive(Serialize)]
pub struct AppInfo {
    version: &'static str,
    repository: String,
    releases: String,
    log_path: Option<String>,
    config_dir: Option<String>,
    updates: config::UpdateSettings,
    platform: &'static str,
}

// [APP-26]
#[tauri::command]
pub fn app_info(state: tauri::State<'_, App>) -> AppInfo {
    let updates = state
        .config
        .lock()
        .map(|c| c.updates.clone())
        .unwrap_or_default();
    AppInfo {
        version: update::CURRENT,
        repository: format!("https://github.com/{}", update::REPO),
        releases: update::releases_page(),
        log_path: journal::path().map(|p| p.display().to_string()),
        config_dir: state.config_path.parent().map(|p| p.display().to_string()),
        updates,
        platform: std::env::consts::OS,
    }
}

// [APP-27]
#[tauri::command]
pub async fn check_update(
    state: tauri::State<'_, App>,
    automatic: bool,
) -> Result<Option<update::UpdateInfo>, String> {
    let skipped = state
        .config
        .lock()
        .ok()
        .and_then(|c| c.updates.skipped.clone());
    let info = tauri::async_runtime::spawn_blocking(update::check)
        .await
        .map_err(|e| e.to_string())?;
    match info {
        Ok(info) => {
            journal::info(format!(
                "mise à jour : version {} en ligne, {} installée",
                info.latest.as_deref().unwrap_or("aucune"),
                info.current
            ));
            let hidden = automatic && (!info.newer || info.latest == skipped);
            Ok((!hidden).then_some(info))
        }
        Err(e) => {
            journal::warn(format!("mise à jour : {e}"));
            if automatic {
                Ok(None)
            } else {
                Err(e)
            }
        }
    }
}

// [APP-28]
#[tauri::command]
pub fn save_updates(
    state: tauri::State<'_, App>,
    check_on_start: Option<bool>,
    skip: Option<String>,
) -> Result<(), String> {
    state.commit(move |cfg| {
        if let Some(v) = check_on_start {
            cfg.updates.check_on_start = v;
        }
        if skip.is_some() {
            cfg.updates.skipped = skip;
        }
    })
}

// [APP-29]
#[tauri::command]
pub fn open_link(url: String) -> Result<(), String> {
    let allowed = format!("https://github.com/{}", update::REPO);
    if url != allowed && !url.starts_with(&format!("{allowed}/")) {
        return Err("Lien non autorisé.".into());
    }
    open::that_detached(&url).map_err(|e| e.to_string())
}

// [APP-30]
#[tauri::command]
pub fn open_folder(state: tauri::State<'_, App>, which: String) -> Result<(), String> {
    let dir = match which.as_str() {
        "config" => state.config_path.parent().map(PathBuf::from),
        "log" => journal::path().and_then(|p| p.parent().map(PathBuf::from)),
        _ => None,
    }
    .ok_or("Dossier inconnu.")?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    open::that_detached(&dir).map_err(|e| e.to_string())
}

// [APP-33]
const IMAGE_EXTENSIONS: [&str; 5] = ["jpg", "jpeg", "png", "webp", "avif"];
// [APP-34]
const MAX_IMAGE_BYTES: u64 = 40 * 1024 * 1024;

// [APP-35]
#[derive(Serialize)]
pub struct AppearanceView {
    settings: AppearanceSettings,
    image: Option<String>,
}

impl App {
    fn config_dir(&self) -> PathBuf {
        self.config_path
            .parent()
            .map(PathBuf::from)
            .unwrap_or_default()
    }

    // [APP-36]
    fn appearance_view(&self) -> AppearanceView {
        let settings = self
            .config
            .lock()
            .map(|c| c.appearance.clone())
            .unwrap_or_default();
        let image = settings
            .background
            .as_ref()
            .map(|name| self.config_dir().join(name))
            .filter(|p| p.is_file())
            .map(|p| p.display().to_string());
        AppearanceView { settings, image }
    }

    fn remove_backgrounds(&self, keep: Option<&str>) {
        let Ok(dir) = std::fs::read_dir(self.config_dir()) else {
            return;
        };
        for e in dir.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with("background-") && Some(name.as_str()) != keep {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
}

// [APP-37]
#[tauri::command]
pub fn get_appearance(state: tauri::State<'_, App>) -> AppearanceView {
    state.appearance_view()
}

// [APP-38]
#[tauri::command]
pub fn save_appearance(
    state: tauri::State<'_, App>,
    appearance: AppearanceSettings,
) -> Result<AppearanceView, String> {
    let appearance = appearance.clamped();
    state.commit(move |cfg| {
        cfg.appearance = AppearanceSettings {
            background: cfg.appearance.background.clone(),
            ..appearance
        }
    })?;
    Ok(state.appearance_view())
}

// [APP-39]
#[tauri::command]
pub fn set_background(
    state: tauri::State<'_, App>,
    path: PathBuf,
) -> Result<AppearanceView, String> {
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .filter(|e| IMAGE_EXTENSIONS.contains(&e.as_str()))
        .ok_or("Format d'image non pris en charge (JPEG, PNG, WebP ou AVIF).")?;
    let size = std::fs::metadata(&path).map_err(|e| e.to_string())?.len();
    if size > MAX_IMAGE_BYTES {
        return Err("Image trop lourde (40 Mo au maximum).".into());
    }
    let name = format!("background-{}.{ext}", crate::controller::now_ms());
    let dir = state.config_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    std::fs::copy(&path, dir.join(&name)).map_err(|e| e.to_string())?;
    let keep = name.clone();
    state.commit(move |cfg| {
        cfg.appearance.background = Some(name);
        cfg.appearance.glass = true;
    })?;
    state.remove_backgrounds(Some(&keep));
    Ok(state.appearance_view())
}

// [APP-40]
#[tauri::command]
pub fn clear_background(state: tauri::State<'_, App>) -> Result<AppearanceView, String> {
    state.commit(|cfg| cfg.appearance.background = None)?;
    state.remove_backgrounds(None);
    Ok(state.appearance_view())
}

// [APP-41]
#[tauri::command]
pub fn save_library_dirs(
    state: tauri::State<'_, App>,
    dirs: Vec<PathBuf>,
) -> Result<SettingsView, String> {
    if let Some(missing) = dirs.iter().find(|p| !p.is_dir()) {
        return Err(format!("Dossier introuvable : {}", missing.display()));
    }
    state.commit(move |cfg| cfg.library.dirs = dirs)?;
    Ok(state.view(None))
}

// [APP-42]
#[tauri::command]
pub fn save_rotation(
    state: tauri::State<'_, App>,
    rotation: blast_library::Rotation,
) -> Result<SettingsView, String> {
    rotation.validate()?;
    let mut rotation = rotation;
    rotation
        .schedule
        .resize(blast_library::SCHEDULE_SLOTS, String::new());
    state.commit(move |cfg| cfg.rotation = rotation)?;
    Ok(state.view(None))
}

// [APP-43]
#[tauri::command]
pub async fn rotation_counts(
    state: tauri::State<'_, App>,
    rules: Vec<blast_library::CategoryRule>,
) -> Result<Vec<usize>, String> {
    let db = state.db_path.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let tracks = blast_library::SqliteStore::open(&db)
            .and_then(|l| l.tracks())
            .map_err(|e| e.to_string())?;
        Ok(rules
            .iter()
            .map(|r| tracks.iter().filter(|t| r.matches(t)).count())
            .collect())
    })
    .await
    .map_err(|e| e.to_string())?
}
