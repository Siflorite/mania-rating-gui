#![windows_subsystem = "windows"]
use anyhow::Result;
use arboard::Clipboard;
use base64::prelude::*;
use chrono::Utc;
use mania_converter::osu_func::{calculate_from_data, OsuDataV128};
use mania_rating_gui::db::{PlayRecord, RatingMapInfo, calc_mod_rating};
use mania_rating_gui::{export_info, prepare_ratings, generate_single_card_pixmap, ScoreTileBase64, RatingInfo};
use osu_db::ModSet;
use resvg::tiny_skia::{IntSize, Pixmap};
use rosu_memory_lib::common::stable::memory::game_state;
use rosu_memory_lib::reader::gameplay::GameplayReader;
use rosu_memory_lib::{init_loop, common::GameState};
use rosu_memory_lib::reader::common::OsuClientKind;
use rosu_memory_lib::reader::resultscreen::ResultScreenReader;
use rosu_memory_lib::reader::beatmap::BeatmapReader;
use slint::{Image, Model, ModelRc, Rgba8Pixel, SharedPixelBuffer, SharedString, VecModel, Weak};
use tokio::task::spawn_blocking;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use rayon::prelude::*;

slint::include_modules!();

#[tokio::main]
async fn main() -> Result<()> {
    let ui = MainWindow::new()?;

    let data = tokio::task::spawn_blocking(|| prepare_ratings("")).await??;
    // let players_list = data.keys().map(SharedString::from).collect::<Vec<_>>();
    let mut players_list = data.keys()
        .filter(|name| *name != "[All Players]" && *name != "[Recent 30]")
        .map(|name| SharedString::from(name)).collect::<Vec<_>>();
    players_list.sort_by(|a, b| a.cmp(b));
    players_list.extend_from_slice(&[SharedString::from("[All Players]"), SharedString::from("[Recent 30]")]);

    let ratings = Arc::new(data);

    ui.set_player_names(ModelRc::new(VecModel::from(players_list)));
    
    // Initialize the model with Recent 30
    update_player_b30(SharedString::from("[Recent 30]"), ratings.clone(), ui.as_weak()).await;

    let ui_handle = ui.as_weak();
    let rating_selection = ratings.clone();
    ui.on_selection_changed(move |player_name| {
        let rating_selection = rating_selection.clone();
        let ui_selection = ui_handle.clone();
        ui_selection.unwrap().set_text_content(SharedString::from("正在加载..."));
        tokio::spawn(update_player_b30(player_name, rating_selection, ui_selection));
    });

    let ui_reset = ui.as_weak();
    let rating_reset = ratings.clone();
    ui.on_reset_tiles(move || {
        let ui = ui_reset.clone();
        let player_name = ui_reset.unwrap().get_current_player_name();
        let rating = rating_reset.clone();
        ui.unwrap().set_text_content(SharedString::from("正在加载..."));
        tokio::spawn(update_player_b30(player_name, rating, ui));
    });

    let ui_remove = ui.as_weak();
    let rating_remove = ratings.clone();
    ui.on_removed(move |index| {
        let ui_handle = ui_remove.clone();
        let player_name = ui_remove.unwrap().get_current_player_name();
        let rating_remove = rating_remove.clone();
        ui_handle.unwrap().set_text_content(SharedString::from("正在加载..."));
        tokio::spawn(remove_tile(player_name, index, rating_remove, ui_handle));
    });

    let ui_add = ui.as_weak();
    let rating_add = ratings.clone();
    ui.on_added(move |index| {
        let ui_handle = ui_add.clone();
        let player_name = ui_add.unwrap().get_current_player_name();
        let rating_add = rating_add.clone();
        ui_handle.unwrap().set_text_content(SharedString::from("正在加载..."));
        tokio::spawn(add_tile(player_name, index, rating_add, ui_handle));
    });

    let ui_export = ui.as_weak();
    let rating_export = ratings.clone();
    ui.on_export(move || {
        let ui_handle = ui_export.clone();
        let player_name = ui_export.unwrap().get_current_player_name();
        let rating_export = rating_export.clone();
        ui_handle.unwrap().set_text_content(SharedString::from("正在导出..."));
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
        let (width, height, bytes) = (raw_data.width() as usize, raw_data.height() as usize, raw_data.as_bytes().to_vec());
        tokio::spawn(copy_image(width, height, bytes));
    });

    let ui_update = ui.as_weak();
    std::thread::spawn(move || {
        update_realtime_text(ui_update)
    });

    ui.run()?;
    Ok(())
}

async fn update_player_b30(player_name: SharedString, rating_selection: Arc<HashMap<String, Vec<RatingInfo>>>, ui_handle: Weak<MainWindow>) {
    let player_ratings = rating_selection.get(player_name.as_str());
    if let Some(rating) = player_ratings {
        let len = rating.len().min(30);
        let slice = rating[0..len].to_vec();
        let pixmaps = spawn_blocking(move || {
            slice.par_iter().enumerate()
                .filter_map(|(i, r)| {
                    generate_single_card_pixmap(i, r).ok()
                })
                .collect::<Vec<_>>()
        }).await.unwrap();
        
        ui_handle.upgrade_in_event_loop(move |ui| {
            let tiles: Vec<ScoreTileData> = pixmaps.iter().enumerate().map(|(i, pixmap)| {
                let pixel_buffer = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(
                    pixmap.data(),
                    pixmap.width(),
                    pixmap.height()
                );
                ScoreTileData { 
                    image: Image::from_rgba8(pixel_buffer),
                    index: i as i32
                }
            }).collect();
            let score_tiles_model = ModelRc::new(VecModel::from(tiles));
            ui.set_score_tiles(score_tiles_model);
            ui.set_removed_tiles(ModelRc::new(VecModel::from(Vec::new())));
            ui.set_text_content(SharedString::from(""));
            ui.set_export_enable(true);
        }).unwrap();
    }
}

async fn remove_tile(player_name: SharedString, index: i32, rating_remove: Arc<HashMap<String, Vec<RatingInfo>>>, ui_handle: Weak<MainWindow>) {
    let player_ratings = rating_remove.get(player_name.as_str());
    if let Some(ratings) = player_ratings {
        let ratings = ratings.clone();
        ui_handle.upgrade_in_event_loop(move |ui| {
            let score_tiles = ui
                .get_score_tiles();
            let score_tiles_vec = score_tiles
                .as_any()
                .downcast_ref::<VecModel<ScoreTileData>>()
                .unwrap();
            if score_tiles_vec.row_count() == 0 {
                return;
            }

            let removed_tiles = ui
                .get_removed_tiles();
            let removed_tiles_vec = removed_tiles
                .as_any()
                .downcast_ref::<VecModel<ScoreTileData>>()
                .unwrap();
            let total = score_tiles_vec.row_count() + removed_tiles_vec.row_count();

            let (real_index, removed_single) = score_tiles_vec
                .iter()
                .enumerate()
                .find(|(_, tile)| {
                    tile.index == index
                })
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
                    new_pixmap.height()
                );
                let new_data = ScoreTileData { 
                    image: Image::from_rgba8(pixel_buffer),
                    index: total as i32
                };
                score_tiles_vec.push(new_data);
            }

            ui.set_text_content(SharedString::from(""));
            ui.set_export_enable(true);
        }).unwrap();
    }
}

async fn add_tile(player_name: SharedString, index: i32, rating_add: Arc<HashMap<String, Vec<RatingInfo>>>, ui_handle: Weak<MainWindow>) {
    let player_ratings = rating_add.get(player_name.as_str());
    if let Some(_ratings) = player_ratings {
        ui_handle.upgrade_in_event_loop(move |ui| {
            let score_tiles = ui
                .get_score_tiles();
            let score_tiles_vec = score_tiles
                .as_any()
                .downcast_ref::<VecModel<ScoreTileData>>()
                .unwrap();

            let removed_tiles = ui
                .get_removed_tiles();
            let removed_tiles_vec = removed_tiles
                .as_any()
                .downcast_ref::<VecModel<ScoreTileData>>()
                .unwrap();
            let (real_index, removed_single) = removed_tiles_vec
                .iter()
                .enumerate()
                .find(|(_, tile)| {
                    tile.index == index
                })
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
        }).unwrap();
    }
}

async fn export(player_name: SharedString, rating_export: Arc<HashMap<String, Vec<RatingInfo>>>, ui_handle: Weak<MainWindow>) {
    let player_ratings = rating_export.get(player_name.as_str());
    if let Some(ratings) = player_ratings {
        let ratings = ratings.clone();
        ui_handle.upgrade_in_event_loop(move |ui: MainWindow| {
            let score_tiles = ui
                .get_score_tiles();
            let score_tiles_vec = score_tiles
                .as_any()
                .downcast_ref::<VecModel<ScoreTileData>>()
                .unwrap();
            let exported_indexes = score_tiles_vec.iter().map(|tile| tile.index as usize).collect::<Vec<_>>();
            let average_rating = ratings.iter()
                .enumerate()
                .filter(|(index, _)| {
                    exported_indexes.contains(index)
                })
                .map(|(_, info)| info.rating)
                .sum::<f64>() / exported_indexes.len() as f64;

            let info_vec = score_tiles_vec
                .iter()
                .enumerate()
                .map(|(index, tile)| ScoreTileBase64 {
                    index: index as i32,
                    base64_string: {
                        let raw_image = tile.image.to_rgba8().unwrap().as_bytes().to_vec();
                        let pixmap = Pixmap::from_vec(raw_image, IntSize::from_wh(1200, 350).unwrap()).unwrap();
                        let png_data = pixmap.encode_png().unwrap();
                        BASE64_STANDARD.encode(&png_data)
                    }
                })
                .collect::<Vec<_>>();

            match export_info(player_name.as_str(), info_vec, average_rating) {
                Ok(path) => {
                    ui.set_text_content(slint::format!("导出完成! 导出路径: {}", path.display()));
                    open::that(path).unwrap();
                }
                Err(e) => {
                    ui.set_text_content(slint::format!("导出失败: {}", e));
                }
            }

            ui.set_export_enable(true);
        }).unwrap();
    } else {
        ui_handle.unwrap().set_export_enable(true);
    }
}

async fn copy_image(width: usize, height: usize, bytes: Vec<u8>) {
    let image_data = arboard::ImageData { 
        width,
        height,
        bytes: bytes.into()
    };

    let mut clipboard = Clipboard::new().unwrap();
    clipboard.set_image(image_data).unwrap();
}

fn update_realtime_text(ui_handle: Weak<MainWindow>) {
    println!("正在尝试读取osu内存");
    ui_handle.upgrade_in_event_loop(|ui| {
        ui.set_test_content(SharedString::from("正在尝试读取osu内存"));
    }).unwrap();

    let (mut state, process) = init_loop(500).unwrap();

    println!("已读取osu内存");
    ui_handle.upgrade_in_event_loop(|ui| {
        ui.set_test_content(SharedString::from("已读取osu内存"));
    }).unwrap();

    let mut new_score_flag = false;
    let mut index = 0;
    let mut mods = ModSet::from_bits(0);

    loop {
        let state_string = format!("当前游戏状态: {:?}", game_state(&process, &mut state));
        ui_handle.upgrade_in_event_loop(|ui| {
            ui.set_test_content(SharedString::from(state_string));
        }).unwrap();
        match game_state(&process, &mut state) {
            Ok(GameState::ResultScreen) => {
                if new_score_flag {
                    println!("Reading result screen");
                    let mut resultscreen_reader =
                        ResultScreenReader::new(&process, &mut state, OsuClientKind::Stable);
                    
                    let mode = resultscreen_reader.mode().unwrap();
                    if mode != rosu_memory_lib::common::GameMode::Mania {
                        println!("{:?}", mode);
                        new_score_flag = false;
                        continue;
                    }
                    let (marv, perf, great, good, bad, miss) = (
                        resultscreen_reader.hits_geki().unwrap_or(0) as u32,
                        resultscreen_reader.hits_300().unwrap_or(0) as u32,
                        resultscreen_reader.hits_katu().unwrap_or(0) as u32,
                        resultscreen_reader.hits_100().unwrap_or(0) as u32,
                        resultscreen_reader.hits_50().unwrap_or(0) as u32,
                        resultscreen_reader.hits_miss().unwrap_or(0) as u32,
                    );
                    let accuracy_rating = (310.0 * marv as f64
                                + 300.0 * perf as f64
                                + 200.0 * great as f64
                                + 100.0 * good as f64
                                + 50.0 * bad as f64)
                                / (3.1 * (marv + perf + great + good + bad + miss) as f64);
                    let player = resultscreen_reader.username().unwrap_or(String::new());
                    let accuracy = resultscreen_reader.accuracy().unwrap_or(0.0);
                    
                    println!("Reading beatmap path");
                    let mut beatmap_reader = BeatmapReader::new(&process, &mut state, OsuClientKind::Stable).unwrap();
                    let mut counts = 5;
                    while beatmap_reader.path().is_err() && counts > 0 {
                        counts -= 1;
                        std::thread::sleep(std::time::Duration::from_millis(100));
                    }
                    if counts == 0 {
                        ui_handle.unwrap().set_test_content(SharedString::from("读取谱面文件失败"));
                        new_score_flag = false;
                        continue;
                    }
                    let path = beatmap_reader.path().unwrap();
                    println!("Parsing beatmap");
                    let beatmap = OsuDataV128::from_file(path.to_str().unwrap()).unwrap().to_legacy();
                    if beatmap.misc.circle_size != 6 || mods.contains(osu_db::Mod::Random) {
                        // Not 6K
                        new_score_flag = false;
                        continue;
                    }
                    let info = beatmap.to_beatmap_info(true);
                    let md5 = beatmap_reader.md5().unwrap();
                    drop(beatmap_reader);
                
                    let timestamp = Utc::now();
                    let play_record = PlayRecord {
                        player,
                        mods,
                        accuracy,
                        accuracy_rating,
                        judgement_num: [marv, perf, great, good, bad, miss],
                        timestamp,
                    };
                    println!("Calculating rating");
                    let sr = info.sr.unwrap_or(0.0);

                    let map_info = RatingMapInfo {
                        hash: md5,
                        path,
                        info,
                    };

                    let sr_ht = if mods.contains(osu_db::Mod::HalfTime) {
                        calculate_from_data(&beatmap, 0.75).unwrap_or(0.0)
                    } else { 0.0 };

                    let sr_dt = if mods.contains(osu_db::Mod::DoubleTime) {
                        calculate_from_data(&beatmap, 1.5).unwrap_or(0.0)
                    } else { 0.0 };

                    let (diff_const, rating) = calc_mod_rating(mods, (sr_ht, sr, sr_dt), accuracy_rating);

                    let rating_info = RatingInfo {
                        map_info: map_info,
                        score_info: play_record,
                        diff_const,
                        rating,
                    };

                    let new_pixmap = generate_single_card_pixmap(index, &rating_info).unwrap();
                    let pixel_buffer = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(
                        new_pixmap.data(),
                        new_pixmap.width(),
                        new_pixmap.height()
                    );
                    
                    println!("Updating UI");
                    ui_handle.upgrade_in_event_loop(move |ui| {
                        let new_data = ScoreTileData { 
                            image: Image::from_rgba8(pixel_buffer),
                            index: index as i32,
                        };
                        let realtime_tiles = ui.get_realtime_tiles();
                        let realtime_tiles_vec = realtime_tiles
                            .as_any()
                            .downcast_ref::<VecModel<ScoreTileData>>()
                            .unwrap();
                        realtime_tiles_vec.push(new_data);
                    }).unwrap();
                    index += 1;
                }
                new_score_flag = false;
            },
            Ok(GameState::Playing) => {
                let mut gameplay_reader = GameplayReader::new(&process, &mut state, OsuClientKind::Stable);
                mods = osu_db::ModSet::from_bits(gameplay_reader.mods().unwrap_or(0));
                new_score_flag = true;
            },
            _ => { 
                new_score_flag = false;
            }
        }
        
        std::thread::sleep(Duration::from_millis(500));
    }
}