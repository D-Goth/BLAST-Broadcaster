#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod api;
mod config;
mod controller;
mod icecast;
mod journal;
mod playlist;
mod secrets;
mod themes;
mod update;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use tauri::Manager;

// [APP-01]
fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let paths = app.path();
    let log_path = paths
        .app_log_dir()
        .map(|d| d.join("blast.log"))
        .unwrap_or_else(|_| PathBuf::from("blast.log"));
    if let Err(e) = journal::init(&log_path) {
        eprintln!("journal {} : {e}", log_path.display());
    }
    let config_path = paths
        .app_config_dir()
        .map(|d| d.join("config.toml"))
        .unwrap_or_else(|_| PathBuf::from("blast-config.toml"));
    let db_path = paths
        .app_data_dir()
        .map(|d| d.join("blast.db"))
        .unwrap_or_else(|_| PathBuf::from("blast.db"));
    let (cfg, warning) = config::load(&config_path);
    if let Some(w) = &warning {
        journal::warn(w);
    }
    let secrets = Arc::new(secrets::Secrets::default());
    let tx = controller::spawn(
        app.handle().clone(),
        secrets.clone(),
        cfg.clone(),
        warning,
        db_path.clone(),
    )?;
    let files: Vec<PathBuf> = std::env::args_os()
        .skip(1)
        .map(PathBuf::from)
        .filter(|p| p.exists())
        .collect();
    if !files.is_empty() {
        let _ = tx.send(controller::UiCommand::AddPaths {
            paths: files,
            index: None,
        });
    }
    let data_dir = db_path
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let config_dir = config_path
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    app.manage(api::App {
        tx,
        config: Mutex::new(cfg),
        config_path,
        db_path,
        secrets,
        ice: icecast::IceManager::new(data_dir, config_dir),
    });
    icecast::autostart(app.handle());
    Ok(())
}

// [APP-02]
fn main() {
    let result = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(setup)
        .invoke_handler(tauri::generate_handler![
            api::blast,
            api::themes,
            api::themes_dir,
            api::get_settings,
            api::save_output,
            api::delete_output,
            api::test_output,
            api::save_aura,
            api::save_chat,
            api::test_aura,
            api::save_audio,
            api::save_voice,
            api::save_input,
            api::save_pad,
            api::clear_pad,
            api::save_sfx_volume,
            api::save_automation,
            api::save_categories,
            api::save_jingles,
            api::export_history,
            api::save_master_preset,
            api::app_info,
            api::check_update,
            api::save_updates,
            api::open_link,
            api::open_folder,
            api::get_appearance,
            api::save_appearance,
            api::set_background,
            api::clear_background,
            api::save_library_dirs,
            api::save_rotation,
            api::rotation_counts,
            icecast::ice_list,
            icecast::ice_detect,
            icecast::ice_save_server,
            icecast::ice_delete_server,
            icecast::ice_status,
            icecast::ice_power,
            icecast::ice_get_config,
            icecast::ice_save_config,
            icecast::ice_adopt_config,
            icecast::ice_new_config,
            icecast::ice_listeners,
            icecast::ice_admin,
            icecast::ice_log,
            icecast::ice_link_output,
            icecast::ice_open_web
        ])
        .build(tauri::generate_context!());
    match result {
        Ok(app) => app.run(on_event),
        Err(e) => {
            journal::error(format!("arrêt anormal : {e}"));
            eprintln!("BLAST Broadcaster : {e}");
            std::process::exit(1);
        }
    }
}

// [APP-44]
fn on_event(handle: &tauri::AppHandle, event: tauri::RunEvent) {
    if !matches!(event, tauri::RunEvent::Exit) {
        return;
    }
    let Some(state) = handle.try_state::<api::App>() else {
        return;
    };
    let servers = state
        .config
        .lock()
        .map(|c| c.icecast.servers.clone())
        .unwrap_or_default();
    state.ice.stop_on_exit(&servers);
    let (done, wait) = crossbeam_channel::bounded(1);
    if state
        .tx
        .send(controller::UiCommand::Shutdown { done })
        .is_ok()
    {
        let _ = wait.recv_timeout(std::time::Duration::from_secs(5));
    }
}
