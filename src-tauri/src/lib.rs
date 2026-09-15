mod commands;
mod embed;
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
            commands::list_folders,
            commands::graph,
            commands::get_graph_config,
            commands::set_graph_config,
            commands::run_base,
            commands::set_base_sort,
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
            commands::get_folds,
            commands::set_folds,
            commands::daily_note,
            commands::templates,
            commands::render_template,
            commands::snippets,
            commands::import_logseq,
            commands::rescan,
            commands::anchor_line,
            commands::attachment_path,
            commands::open_external,
            commands::record_event,
            commands::related,
            commands::forget_memory,
            commands::all_properties,
            commands::embed_status,
            commands::typing,
            commands::semantic_edges,
            commands::set_model_dir,
        ])
        .run(tauri::generate_context!())
        .expect("error while running engram-notes");
}
