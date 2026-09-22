mod agent_api;
mod agents;
mod assets;
mod bank;
mod bins;
mod blender;
mod catalog;
mod compiler;
mod git_tools;
mod keys;
mod oauth;
mod collab;
mod projects;
mod publish;
mod review;
mod rojo;
mod studio;
mod swarm;
mod sync;
mod textures;

use agents::SessionMap;
use compiler::CompilerState;
use rojo::RojoState;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use studio::{OfferState, PlaceOffer, StudioHeartbeat};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let sessions: SessionMap = Arc::new(Mutex::new(HashMap::new()));
    let studio_state: studio::StudioState = Arc::new(Mutex::new(StudioHeartbeat::default()));
    let offer_state: OfferState = Arc::new(Mutex::new(PlaceOffer::default()));
    let rojo_state = Mutex::new(RojoState::default());
    let compiler_state = Mutex::new(CompilerState::default());

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .manage(sessions)
        .manage(studio_state.clone())
        .manage(offer_state.clone())
        .manage(rojo_state)
        .manage(compiler_state)
        .setup(move |app| {
            studio::spawn_server(app.handle().clone(), studio_state.clone(), offer_state.clone());
            agent_api::spawn_server(app.handle().clone());
            std::thread::spawn(git_tools::ensure_binaries);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            keys::get_keys,
            keys::set_keys,
            oauth::get_roblox_user,
            oauth::start_roblox_login,
            oauth::logout_roblox,
            projects::list_projects,
            projects::create_project,
            collab::project_share_status,
            collab::share_project,
            collab::push_project,
            collab::pull_project,
            collab::join_project,
            projects::open_project_dir,
            projects::set_reference_projects,
            agents::detect_agents,
            agents::start_agent,
            agents::write_agent,
            agents::resize_agent,
            agents::stop_agent,
            agents::live_agents,
            agents::pause_project,
            swarm::load_swarm,
            swarm::save_swarm,
            assets::generate_image,
            assets::generate_mesh,
            assets::poll_mesh,
            blender::detect_blender,
            blender::run_blender_mesh_cmd,
            assets::save_image_to_project,
            assets::save_pasted_image,
            assets::save_mesh_url,
            rojo::rojo_status,
            rojo::start_rojo,
            rojo::stop_rojo,
            rojo::install_rojo,
            compiler::compiler_status,
            compiler::toolchain_status,
            compiler::start_sync,
            compiler::stop_sync,
            studio::studio_status,
            studio::sync_offer,
            studio::remind_studio,
            studio::bind_open_place,
            studio::install_studio_plugin,
            bank::list_bank,
            bank::bank_counts,
            bank::bank_page,
            bank::read_lumen_file,
            bank::save_mesh_preview,
            bank::export_lumen_bank,
            bank::import_lumen_bank,
            sync::sync_shared_bank,
            catalog::list_vibestarter_bank,
            catalog::cache_remote_asset,
            textures::list_textures_bank,
            textures::add_studio_texture,
            textures::import_studio_textures,
            textures::set_texture_tiling,
            publish::import_to_bank,
            publish::publish_bank_item,
            review::pending_asset_reviews,
            review::resolve_asset_review,
            review::pending_library_choices,
            review::resolve_library_choice,
        ])
        .build(tauri::generate_context!())
        .expect("erreur au démarrage de Lumen")
        .run(|app, event| {
            if let tauri::RunEvent::ExitRequested { .. } = event {
                agents::pause_all_on_exit(app);
            }
        });
}
