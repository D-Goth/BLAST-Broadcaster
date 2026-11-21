#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod controller;
mod playlist;
mod themes;

use crossbeam_channel::Sender;
use tauri::Manager;

struct Ui(Sender<controller::UiCommand>);

#[tauri::command]
fn blast(state: tauri::State<'_, Ui>, cmd: controller::UiCommand) -> Result<(), String> {
    state.0.send(cmd).map_err(|e| e.to_string())
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

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let tx = controller::spawn(app.handle().clone());
            let paths: Vec<std::path::PathBuf> = std::env::args_os()
                .skip(1)
                .map(std::path::PathBuf::from)
                .filter(|p| p.exists())
                .collect();
            if !paths.is_empty() {
                let _ = tx.send(controller::UiCommand::AddPaths { paths, index: None });
            }
            app.manage(Ui(tx));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![blast, themes, themes_dir])
        .run(tauri::generate_context!())
        .expect("BLAST Broadcaster");
}
