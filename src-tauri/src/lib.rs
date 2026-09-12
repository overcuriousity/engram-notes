mod commands;
mod error;
mod state;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(state::AppState::default())
        .invoke_handler(tauri::generate_handler![
            commands::recent_vaults,
            commands::startup_vault,
            commands::open_vault,
            commands::list_files,
            commands::read_note,
            commands::write_note,
            commands::create_note,
            commands::create_folder,
            commands::delete_file,
            commands::plan_rename,
            commands::apply_rename,
            commands::backlinks,
            commands::outgoing,
            commands::unresolved,
            commands::resolve_link,
            commands::titles,
            commands::tags,
            commands::properties,
            commands::set_property,
            commands::search,
            commands::get_config,
            commands::set_config,
            commands::get_workspace,
            commands::set_workspace,
            commands::daily_note,
            commands::rescan,
            commands::anchor_line,
        ])
        .run(tauri::generate_context!())
        .expect("error while running engram-notes");
}
