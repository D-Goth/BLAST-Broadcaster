#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod config;
mod controller;
mod playlist;
mod secrets;
mod themes;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use blast_aura::AuraSettings;
use blast_cast::OutputConfig;
use config::{AppConfig, AudioSettings, PadConfig, SoundboardSettings, VoiceSettings, PAD_COUNT};
use crossbeam_channel::Sender;
use secrets::{output_key, Secrets, Stored, AURA_KEY};
use serde::Serialize;
use tauri::Manager;

struct App {
    tx: Sender<controller::UiCommand>,
    config: Mutex<AppConfig>,
    config_path: PathBuf,
    secrets: Arc<Secrets>,
}

#[derive(Serialize)]
struct OutputView {
    #[serde(flatten)]
    cfg: OutputConfig,
    has_password: bool,
}

#[derive(Serialize)]
struct SettingsView {
    outputs: Vec<OutputView>,
    aura: AuraSettings,
    aura_has_password: bool,
    audio: AudioSettings,
    voice: VoiceSettings,
    soundboard: SoundboardSettings,
    devices: Vec<String>,
    default_device: Option<String>,
    input_devices: Vec<String>,
    default_input: Option<String>,
    secret_store: String,
    config_path: String,
    warning: Option<String>,
}

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
            voice: cfg.voice,
            soundboard: cfg.soundboard,
            devices: blast_core::engine::list_output_devices().unwrap_or_default(),
            default_device: blast_core::engine::default_output_device(),
            input_devices: blast_core::engine::list_input_devices().unwrap_or_default(),
            default_input: blast_core::engine::default_input_device(),
            secret_store: self.secrets.store_label(),
            config_path: self.config_path.display().to_string(),
            warning,
        }
    }

    fn commit(&self, edit: impl FnOnce(&mut AppConfig)) -> Result<(), String> {
        let mut cfg = self.config.lock().map_err(|e| e.to_string())?;
        edit(&mut cfg);
        config::save(&self.config_path, &cfg)?;
        let _ = self
            .tx
            .send(controller::UiCommand::Config(Box::new(cfg.clone())));
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

#[tauri::command]
fn blast(state: tauri::State<'_, App>, cmd: controller::UiCommand) -> Result<(), String> {
    state.tx.send(cmd).map_err(|e| e.to_string())
}

#[tauri::command]
fn themes(app: tauri::AppHandle) -> Vec<serde_json::Value> {
    let dir = app.path().app_config_dir().ok().map(|d| d.join("themes"));
    themes::list(dir.as_deref())
}

#[tauri::command]
fn themes_dir(app: tauri::AppHandle) -> Option<String> {
    let dir = app.path().app_config_dir().ok()?.join("themes");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir.to_string_lossy().into_owned())
}

#[tauri::command]
fn get_settings(state: tauri::State<'_, App>) -> SettingsView {
    state.view(None)
}

#[tauri::command]
fn save_output(
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

#[tauri::command]
fn delete_output(state: tauri::State<'_, App>, id: String) -> Result<SettingsView, String> {
    state.secrets.delete(&output_key(&id));
    state.commit(|cfg| cfg.outputs.retain(|o| o.id != id))?;
    Ok(state.view(None))
}

#[tauri::command]
async fn test_output(
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

#[tauri::command]
fn save_aura(
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

#[tauri::command]
async fn test_aura(
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

#[tauri::command]
fn save_audio(
    state: tauri::State<'_, App>,
    device: Option<String>,
) -> Result<SettingsView, String> {
    let device = device.filter(|d| !d.is_empty());
    state.commit(move |cfg| cfg.audio.device = device)?;
    Ok(state.view(Some(
        "La nouvelle sortie audio sera utilisée au prochain démarrage de BLAST.".into(),
    )))
}

#[tauri::command]
fn save_voice(state: tauri::State<'_, App>, voice: VoiceSettings) -> Result<(), String> {
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

#[tauri::command]
fn save_input(
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

#[tauri::command]
fn save_pad(state: tauri::State<'_, App>, pad: PadConfig) -> Result<(), String> {
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

#[tauri::command]
fn clear_pad(state: tauri::State<'_, App>, index: u16) -> Result<(), String> {
    state.commit(move |cfg| cfg.soundboard.pads.retain(|p| p.index != index))
}

#[tauri::command]
fn save_sfx_volume(state: tauri::State<'_, App>, volume_db: f32) -> Result<(), String> {
    state.commit(move |cfg| cfg.soundboard.volume_db = volume_db.clamp(-60.0, 6.0))
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let config_path = app
                .path()
                .app_config_dir()
                .map(|d| d.join("config.toml"))
                .unwrap_or_else(|_| PathBuf::from("blast-config.toml"));
            let (cfg, warning) = config::load(&config_path);
            let secrets = Arc::new(Secrets::default());
            let tx = controller::spawn(app.handle().clone(), secrets.clone(), cfg.clone(), warning);
            let paths: Vec<PathBuf> = std::env::args_os()
                .skip(1)
                .map(PathBuf::from)
                .filter(|p| p.exists())
                .collect();
            if !paths.is_empty() {
                let _ = tx.send(controller::UiCommand::AddPaths { paths, index: None });
            }
            app.manage(App {
                tx,
                config: Mutex::new(cfg),
                config_path,
                secrets,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            blast,
            themes,
            themes_dir,
            get_settings,
            save_output,
            delete_output,
            test_output,
            save_aura,
            test_aura,
            save_audio,
            save_voice,
            save_input,
            save_pad,
            clear_pad,
            save_sfx_volume
        ])
        .run(tauri::generate_context!())
        .expect("BLAST Broadcaster");
}
