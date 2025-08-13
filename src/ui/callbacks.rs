use crate::db::RatingInfo;
use crate::graphx::{export_info, generate_single_card_pixmap};
use crate::ui::ScoreTileBase64;
use crate::{MainWindow, ScoreTileData};
use arboard::Clipboard;
use base64::prelude::*;
use native_dialog::{DialogBuilder, MessageLevel};
use rayon::prelude::*;
use resvg::tiny_skia::{IntSize, Pixmap};
use slint::{Image, Model, ModelRc, Rgba8Pixel, SharedPixelBuffer, SharedString, VecModel, Weak};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::task::spawn_blocking;

pub async fn update_player_b30(
    player_name: SharedString,
    rating_selection: Arc<Mutex<HashMap<String, Vec<RatingInfo>>>>,
    ui_handle: Weak<MainWindow>,
) {
    let player_ratings = {
        let r = rating_selection.lock().unwrap();
        r.get(player_name.as_str()).cloned()
    };
    if let Some(rating) = player_ratings {
        let len = rating.len().min(30);
        let slice = rating[0..len].to_vec();
        let pixmaps = spawn_blocking(move || {
            slice
                .par_iter()
                .enumerate()
                .filter_map(|(i, r)| generate_single_card_pixmap(i, r).ok())
                .collect::<Vec<_>>()
        })
        .await
        .unwrap();

        ui_handle
            .upgrade_in_event_loop(move |ui| {
                let tiles: Vec<ScoreTileData> = pixmaps
                    .iter()
                    .enumerate()
                    .map(|(i, pixmap)| {
                        let pixel_buffer = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(
                            pixmap.data(),
                            pixmap.width(),
                            pixmap.height(),
                        );
                        ScoreTileData {
                            image: Image::from_rgba8(pixel_buffer),
                            index: i as i32,
                        }
                    })
                    .collect();
                let score_tiles_model = ModelRc::new(VecModel::from(tiles));
                ui.set_score_tiles(score_tiles_model);
                ui.set_removed_tiles(ModelRc::new(VecModel::from(Vec::new())));
                ui.set_text_content(SharedString::from(""));
                ui.set_export_enable(true);
            })
            .unwrap();
    }
}

pub async fn remove_tile(
    player_name: SharedString,
    index: i32,
    rating_remove: Arc<Mutex<HashMap<String, Vec<RatingInfo>>>>,
    ui_handle: Weak<MainWindow>,
) {
    let player_ratings = {
        let r = rating_remove.lock().unwrap();
        r.get(player_name.as_str()).cloned()
    };
    if let Some(ratings) = player_ratings {
        let ratings = ratings.clone();
        ui_handle
            .upgrade_in_event_loop(move |ui| {
                let score_tiles = ui.get_score_tiles();
                let score_tiles_vec = score_tiles
                    .as_any()
                    .downcast_ref::<VecModel<ScoreTileData>>()
                    .unwrap();
                if score_tiles_vec.row_count() == 0 {
                    return;
                }

                let removed_tiles = ui.get_removed_tiles();
                let removed_tiles_vec = removed_tiles
                    .as_any()
                    .downcast_ref::<VecModel<ScoreTileData>>()
                    .unwrap();
                let total = score_tiles_vec.row_count() + removed_tiles_vec.row_count();

                let (real_index, removed_single) = score_tiles_vec
                    .iter()
                    .enumerate()
                    .find(|(_, tile)| tile.index == index)
                    .unwrap();
                score_tiles_vec.remove(real_index);
                removed_tiles_vec.push(removed_single);
                // 最好排一下序
                let mut removed_vec_data = removed_tiles_vec.iter().collect::<Vec<_>>();
                removed_vec_data.sort_by(|a, b| a.index.cmp(&b.index));
                removed_tiles_vec.set_vec(removed_vec_data);

                // 寻找不在当前列表中的第一个rating
                // 实际上就是score_tiles和removed_tiles的总数
                if ratings.len() > total {
                    let new_info = &ratings[total];
                    let new_pixmap = generate_single_card_pixmap(total, new_info).unwrap();
                    let pixel_buffer = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(
                        new_pixmap.data(),
                        new_pixmap.width(),
                        new_pixmap.height(),
                    );
                    let new_data = ScoreTileData {
                        image: Image::from_rgba8(pixel_buffer),
                        index: total as i32,
                    };
                    score_tiles_vec.push(new_data);
                }

                ui.set_text_content(SharedString::from(""));
                ui.set_export_enable(true);
            })
            .unwrap();
    }
}

pub async fn add_tile(
    player_name: SharedString,
    index: i32,
    rating_add: Arc<Mutex<HashMap<String, Vec<RatingInfo>>>>,
    ui_handle: Weak<MainWindow>,
) {
    let player_ratings = {
        let r = rating_add.lock().unwrap();
        r.get(player_name.as_str()).cloned()
    };
    if let Some(_ratings) = player_ratings {
        ui_handle
            .upgrade_in_event_loop(move |ui| {
                let score_tiles = ui.get_score_tiles();
                let score_tiles_vec = score_tiles
                    .as_any()
                    .downcast_ref::<VecModel<ScoreTileData>>()
                    .unwrap();

                let removed_tiles = ui.get_removed_tiles();
                let removed_tiles_vec = removed_tiles
                    .as_any()
                    .downcast_ref::<VecModel<ScoreTileData>>()
                    .unwrap();
                let (real_index, removed_single) = removed_tiles_vec
                    .iter()
                    .enumerate()
                    .find(|(_, tile)| tile.index == index)
                    .unwrap();

                // removed_tiles_vec去除real_index，score_tiles_vec去除最后一个，再push进去 removed_single
                removed_tiles_vec.remove(real_index);
                if score_tiles_vec.row_count() == 30 {
                    score_tiles_vec.remove(score_tiles_vec.row_count() - 1);
                }
                score_tiles_vec.push(removed_single);

                // 最好排一下序
                let mut score_vec_data = score_tiles_vec.iter().collect::<Vec<_>>();
                score_vec_data.sort_by(|a, b| a.index.cmp(&b.index));
                score_tiles_vec.set_vec(score_vec_data);

                ui.set_text_content(SharedString::from(""));
                ui.set_export_enable(true);
            })
            .unwrap();
    }
}

pub async fn export(
    player_name: SharedString,
    rating_export: Arc<Mutex<HashMap<String, Vec<RatingInfo>>>>,
    ui_handle: Weak<MainWindow>,
) {
    let player_ratings = {
        let r = rating_export.lock().unwrap();
        r.get(player_name.as_str()).cloned()
    };
    if let Some(ratings) = player_ratings {
        let ratings = ratings.clone();
        ui_handle
            .upgrade_in_event_loop(move |ui: MainWindow| {
                let score_tiles = ui.get_score_tiles();
                let score_tiles_vec = score_tiles
                    .as_any()
                    .downcast_ref::<VecModel<ScoreTileData>>()
                    .unwrap();
                let exported_indexes = score_tiles_vec
                    .iter()
                    .map(|tile| tile.index as usize)
                    .collect::<Vec<_>>();
                let average_rating = ratings
                    .par_iter()
                    .enumerate()
                    .filter(|(index, _)| exported_indexes.contains(index))
                    .map(|(_, info)| info.rating)
                    .sum::<f64>()
                    / exported_indexes.len() as f64;

                let info_vec = score_tiles_vec
                    .iter()
                    .enumerate()
                    .map(|(index, tile)| ScoreTileBase64 {
                        index: index as i32,
                        base64_string: {
                            let raw_image = tile.image.to_rgba8().unwrap().as_bytes().to_vec();
                            let pixmap =
                                Pixmap::from_vec(raw_image, IntSize::from_wh(1200, 350).unwrap())
                                    .unwrap();
                            let png_data = pixmap.encode_png().unwrap();
                            BASE64_STANDARD.encode(&png_data)
                        },
                    })
                    .collect::<Vec<_>>();

                match export_info(player_name.as_str(), info_vec, average_rating) {
                    Ok(path) => {
                        ui.set_text_content(slint::format!(
                            "导出完成! 导出路径: {}",
                            path.display()
                        ));
                        open::that(path).unwrap();
                    }
                    Err(e) => {
                        ui.set_text_content(slint::format!("导出失败: {}", e));
                    }
                }

                ui.set_export_enable(true);
            })
            .unwrap();
    } else {
        ui_handle.unwrap().set_export_enable(true);
    }
}

pub async fn copy_image(width: usize, height: usize, bytes: Vec<u8>) {
    let image_data = arboard::ImageData {
        width,
        height,
        bytes: bytes.into(),
    };

    let mut clipboard = Clipboard::new().unwrap();
    clipboard.set_image(image_data).unwrap();
}

pub fn select_osu_folder(has_available_path: bool) -> String {
    loop {
        let path = DialogBuilder::file()
            .set_location("~/Desktop")
            .set_title("选择osu!文件夹")
            .open_single_dir()
            .show()
            .unwrap();
        println!("{path:?}");
        if let Some(path) = path {
            let scores_db_path = path.join("scores.db");
            let osu_db_path = path.join("osu!.db");
            if scores_db_path.exists() && osu_db_path.exists() {
                return path.to_string_lossy().into_owned();
            }
        }
        DialogBuilder::message()
            .set_level(MessageLevel::Error)
            .set_title("Cannot find db")
            .set_text("该路径下没有osu!.db和scores.db文件!")
            .confirm()
            .show()
            .unwrap();
        if has_available_path {
            return String::new();
        }
    }
}
