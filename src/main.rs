#![windows_subsystem = "windows"]
pub mod db;
pub mod graphx;
pub mod ui;

use crate::db::{RatingInfo, get_osu_install_path, prepare_ratings};
use crate::ui::bs::update_realtime;
use crate::ui::callbacks::{
    add_tile, copy_image, export, remove_tile, select_osu_folder, update_player_b30,
};
use crate::ui::{ScoreTileBase64, ThreadManager};
use anyhow::Result;
use slint::{ModelRc, SharedString, VecModel, Weak};
use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};

slint::include_modules!();
type LazyScoreMap = LazyLock<Arc<Mutex<HashMap<String, Vec<RatingInfo>>>>>;
static SCORES_DATA: LazyScoreMap = LazyLock::new(|| Arc::new(Mutex::new(HashMap::new())));

#[tokio::main]
async fn main() -> Result<()> {
    let ui = MainWindow::new()?;
    let osu_path = get_osu_install_path();
    // let osu_path: Option<std::path::PathBuf> = None; // For testing
    let osu_exe_dir = match osu_path {
        Some(p) => p.to_string_lossy().into_owned(),
        None => select_osu_folder().ok_or(anyhow::Error::msg("Cannot find osu directory"))?,
    };
    ui.set_osu_dir(SharedString::from(&osu_exe_dir));
    initialize(osu_exe_dir.clone(), ui.as_weak()).await?;

    let ratings = SCORES_DATA.clone();

    let ui_handle = ui.as_weak();
    let rating_selection = ratings.clone();
    ui.on_selection_changed(move |player_name| {
        let rating_selection = rating_selection.clone();
        let ui_selection = ui_handle.clone();
        ui_selection
            .unwrap()
            .set_text_content(SharedString::from("正在加载..."));
        tokio::spawn(update_player_b30(
            player_name,
            rating_selection,
            ui_selection,
        ));
    });

    let ui_reset = ui.as_weak();
    let rating_reset = ratings.clone();
    ui.on_reset_tiles(move || {
        let ui = ui_reset.clone();
        let player_name = ui_reset.unwrap().get_current_player_name();
        let rating = rating_reset.clone();
        ui.unwrap()
            .set_text_content(SharedString::from("正在加载..."));
        tokio::spawn(update_player_b30(player_name, rating, ui));
    });

    let ui_remove = ui.as_weak();
    let rating_remove = ratings.clone();
    ui.on_removed(move |index| {
        let ui_handle = ui_remove.clone();
        let player_name = ui_remove.unwrap().get_current_player_name();
        let rating_remove = rating_remove.clone();
        ui_handle
            .unwrap()
            .set_text_content(SharedString::from("正在加载..."));
        tokio::spawn(remove_tile(player_name, index, rating_remove, ui_handle));
    });

    let ui_add = ui.as_weak();
    let rating_add = ratings.clone();
    ui.on_added(move |index| {
        let ui_handle = ui_add.clone();
        let player_name = ui_add.unwrap().get_current_player_name();
        let rating_add = rating_add.clone();
        ui_handle
            .unwrap()
            .set_text_content(SharedString::from("正在加载..."));
        tokio::spawn(add_tile(player_name, index, rating_add, ui_handle));
    });

    let ui_export = ui.as_weak();
    let rating_export = ratings.clone();
    ui.on_export(move || {
        let ui_handle = ui_export.clone();
        let player_name = ui_export.unwrap().get_current_player_name();
        let rating_export = rating_export.clone();
        ui_handle
            .unwrap()
            .set_text_content(SharedString::from("正在导出..."));
        tokio::spawn(export(player_name, rating_export, ui_handle));
    });

    ui.on_show_help_window(move || {
        let help_window = HelpWindow::new().unwrap();
        help_window.show().unwrap();
        help_window.on_open_help_url(move || {
            open::that("https://github.com/Siflorite/mania-rating-gui").unwrap();
        });
    });

    ui.on_copied(move |image| {
        let raw_data = image.to_rgba8().unwrap();
        let (width, height, bytes) = (
            raw_data.width() as usize,
            raw_data.height() as usize,
            raw_data.as_bytes().to_vec(),
        );
        tokio::spawn(copy_image(width, height, bytes));
    });

    let ui_update = ui.as_weak();
    let mut thread_manager = ThreadManager::new();
    ui.on_toggle_realtime(move |status| {
        let ui_update = ui_update.clone();
        if status {
            thread_manager.start_thread(update_realtime, ui_update);
        } else {
            thread_manager.stop_thread();
        }
    });

    // let thread_manager = ThreadManagerAsync::new();
    // let manager_lock = Arc::new(Mutex::new(thread_manager));
    // ui.on_toggle_realtime(move |status| {
    //     let ui_update = ui_update.clone();
    //     let manager_lock = manager_lock.clone();
    //     tokio::spawn(async move {
    //         if status {
    //             manager_lock.lock().await.start_thread(update_realtime_async, ui_update).await;
    //         } else {
    //             manager_lock.lock().await.stop_thread().await;
    //         }
    //     });
    // });

    let ui_refresh = ui.as_weak();
    ui.on_refresh(move || {
        ui_refresh.unwrap().window().request_redraw();
    });

    let ui_s = ui.as_weak();
    ui.on_select_osu_dir(move || {
        let new_dir = select_osu_folder().unwrap_or_default();
        if !new_dir.is_empty() && new_dir != osu_exe_dir {
            ui_s.unwrap().set_osu_dir(SharedString::from(&new_dir));
            let ui_s = ui_s.clone();
            tokio::spawn(initialize(new_dir, ui_s));
        } else {
            ui_s.unwrap().set_folder_select_enable(true);
        }
    });

    ui.run()?;
    Ok(())
}

pub async fn initialize(osu_exe_dir: String, ui: Weak<MainWindow>) -> Result<()> {
    let data = prepare_ratings(&osu_exe_dir)?;
    let mut players_list = {
        let mut scores = SCORES_DATA.lock().unwrap();
        scores.clear();
        for (k, v) in data {
            scores.insert(k, v);
        }
        // let players_list = data.keys().map(SharedString::from).collect::<Vec<_>>();
        scores
            .keys()
            .filter(|name| *name != "[All Players]" && *name != "[Recent 30]")
            .map(SharedString::from)
            .collect::<Vec<_>>()
    };
    players_list.sort();
    players_list.extend_from_slice(&[
        SharedString::from("[All Players]"),
        SharedString::from("[Recent 30]"),
    ]);
    ui.upgrade_in_event_loop(|ui| {
        ui.set_player_names(ModelRc::new(VecModel::from(players_list)));
        ui.set_folder_select_enable(true);
    })
    .unwrap();
    // Initialize the model with Recent 30
    update_player_b30(SharedString::from("[Recent 30]"), SCORES_DATA.clone(), ui).await;
    Ok(())
}
