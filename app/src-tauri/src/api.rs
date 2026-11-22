use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use blast_aura::AuraSettings;
use blast_cast::OutputConfig;
use crossbeam_channel::Sender;
use serde::Serialize;
use tauri::Manager;

use crate::config::{
    self, AppConfig, AudioSettings, AutomationSettings, PadConfig, SoundboardSettings,
    VoiceSettings, PAD_COUNT,
};
use crate::controller::UiCommand;
use crate::secrets::{output_key, Secrets, Stored, AURA_KEY};
use crate::{journal, themes, update};

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
            library: blast_library::Library::open(&self.db_path)
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
    state.commit(move |cfg| cfg.automation = automation)?;
    Ok(state.view(None))
}

// [APP-24]
#[tauri::command]
pub fn export_history(state: tauri::State<'_, App>, path: String) -> Result<usize, String> {
    let lib = blast_library::Library::open(&state.db_path).map_err(|e| e.to_string())?;
    let rows = lib.recent(u32::MAX).map_err(|e| e.to_string())?;
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
