// This is a part for wiping the stinky asses of dumb crate providers like rosu-memory-lib
use crate::db::{PlayRecord, RatingInfo, RatingMapInfo, calc_mod_rating};
use crate::graphx::generate_single_card_pixmap;
use crate::{MainWindow, ScoreTileData};
use anyhow::{Result, bail};
use chrono::Utc;
use mania_converter::osu_func::{OsuDataV128, calculate_from_data};
use osu_db::ModSet;
use rosu_mem::process::{Process, ProcessTraits};
use rosu_memory_lib::common::GameState;
use rosu_memory_lib::common::stable::memory::game_state;
use rosu_memory_lib::reader::beatmap::BeatmapReader;
use rosu_memory_lib::reader::common::OsuClientKind;
use rosu_memory_lib::reader::gameplay::GameplayReader;
use rosu_memory_lib::reader::resultscreen::ResultScreenReader;
use rosu_memory_lib::reader::structs::{State, StaticAddresses};
use slint::{
    ComponentHandle, Image, Model, Rgba8Pixel, SharedPixelBuffer, SharedString, VecModel, Weak,
};
use std::sync::{Arc, Mutex};
use std::time::Duration;

// The original fking `init_loop()` just goes into a dead loop if osu!.exe is not booted
// And the thread generated whether by std::thread or tokio just won't fking terminate
// So I have to implement a version where I can control the stop signal myself
// Will generate an async version later
#[allow(dead_code)]
fn init_loop_with_flag(sleep_duration: u64, flag: Arc<Mutex<bool>>) -> Result<(State, Process)> {
    let mut state = State {
        addresses: StaticAddresses::default(),
    };

    loop {
        if !(*flag.lock().unwrap()) {
            bail!("Force exit");
        }
        if let Some(v) = init_loop_inner(&mut state, sleep_duration) {
            return Ok((state, v));
        }
        std::thread::sleep(Duration::from_millis(sleep_duration));
    }
}

#[allow(dead_code)]
async fn init_loop_async(sleep_duration: u64) -> (State, Process) {
    let mut state = State {
        addresses: StaticAddresses::default(),
    };

    loop {
        if let Some(v) = init_loop_inner(&mut state, sleep_duration) {
            return (state, v);
        }
        tokio::time::sleep(Duration::from_millis(sleep_duration)).await;
    }
}

fn init_loop_inner(state: &mut State, sleep_duration: u64) -> Option<Process> {
    match Process::initialize("osu!.exe", &["umu-run", "waitforexitandrun"]) {
        Ok(p) => {
            println!("Found process, pid - {}", p.pid);
            println!("Reading static signatures...");
            match StaticAddresses::new(&p) {
                Ok(v) => {
                    state.addresses = v;
                    println!("Static addresses read successfully");
                    return Some(p);
                }
                Err(e) => match e {
                    rosu_memory_lib::error::Error::MemoryRead(msg) => {
                        if msg.contains("Process not found") {
                            println!("Process not found, sleeping for {sleep_duration}ms");
                        }
                        #[cfg(target_os = "windows")]
                        if msg.contains("OS error") {
                            println!("OS error, sleeping for {sleep_duration}ms");
                        }
                        println!("Unknown error, sleeping for {sleep_duration}ms");
                    }
                    _ => {
                        println!("Unknown error, sleeping for {sleep_duration}ms");
                    }
                },
            }
        }
        Err(_) => {
            println!("Unknown process error, sleeping for {sleep_duration}ms");
        }
    }
    None
}

pub async fn update_realtime_async(
    ui_handle: Weak<MainWindow>,
    flag: Arc<tokio::sync::watch::Receiver<bool>>,
) {
    println!("正在尝试读取osu内存");
    ui_handle
        .upgrade_in_event_loop(|ui| {
            ui.set_test_content(SharedString::from("正在尝试读取osu内存"));
        })
        .unwrap();

    let (mut state, process) = init_loop_async(500).await;

    println!("已读取osu内存");
    ui_handle
        .upgrade_in_event_loop(|ui| {
            ui.set_test_content(SharedString::from("已读取osu内存"));
        })
        .unwrap();

    let mut new_score_flag = false;
    let mut mods = ModSet::from_bits(0);
    let mut prev_state = GameState::Unknown;

    loop {
        {
            if !*flag.borrow() {
                println!("exit loop");
                break;
            }
        }
        update_realtime_inner(
            &process,
            &mut state,
            &mut prev_state,
            &mut new_score_flag,
            &mut mods,
            ui_handle.clone(),
        );
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

pub fn update_realtime(ui_handle: Weak<MainWindow>, flag: Arc<Mutex<bool>>) {
    println!("正在尝试读取osu内存");
    ui_handle
        .upgrade_in_event_loop(|ui| {
            ui.set_test_content(SharedString::from("正在尝试读取osu内存"));
        })
        .unwrap();

    // init_loop is a total BS which does a loop that never ends, so I had to do the same function myself...
    let (mut state, process) = match init_loop_with_flag(500, flag.clone()) {
        Ok((state, process)) => (state, process),
        Err(e) => {
            println!("Error: {e}");
            return;
        }
    };

    println!("已读取osu内存");
    ui_handle
        .upgrade_in_event_loop(|ui| {
            ui.set_test_content(SharedString::from("已读取osu内存"));
        })
        .unwrap();

    let mut new_score_flag = false;
    let mut mods = ModSet::from_bits(0);
    let mut prev_state = GameState::Unknown;

    loop {
        {
            if !*flag.lock().unwrap() {
                println!("exit loop");
                break;
            }
        }
        update_realtime_inner(
            &process,
            &mut state,
            &mut prev_state,
            &mut new_score_flag,
            &mut mods,
            ui_handle.clone(),
        );
        std::thread::sleep(Duration::from_millis(500));
    }
}

fn update_realtime_inner(
    process: &Process,
    state: &mut State,
    prev_state: &mut GameState,
    new_score_flag: &mut bool,
    mods: &mut ModSet,
    ui_handle: Weak<MainWindow>,
) {
    let gamestate = game_state(process, state).unwrap_or(GameState::Unknown);
    if gamestate != *prev_state {
        let state_string = format!("当前游戏状态: {gamestate:?}");
        ui_handle
            .upgrade_in_event_loop(|ui| {
                ui.set_test_content(SharedString::from(state_string));
            })
            .unwrap();
        *prev_state = gamestate;
    }

    match game_state(process, state) {
        Ok(GameState::ResultScreen) => {
            if *new_score_flag {
                println!("Reading result screen");
                let mut resultscreen_reader =
                    ResultScreenReader::new(process, state, OsuClientKind::Stable);

                match resultscreen_reader.mode() {
                    Ok(rosu_memory_lib::common::GameMode::Mania) => {}
                    Ok(rosu_memory_lib::common::GameMode::Unknown) => {
                        println!("Read fails, retrying...");
                        return;
                    }
                    Ok(mode) => {
                        println!("Unsupported game mode: {mode:?}");
                        *new_score_flag = false;
                        return;
                    }
                    Err(e) => {
                        println!("Error: {e}");
                        return;
                    }
                }
                let (marv, perf, great, good, bad, miss) = (
                    resultscreen_reader.hits_geki().unwrap_or(0) as u32,
                    resultscreen_reader.hits_300().unwrap_or(0) as u32,
                    resultscreen_reader.hits_katu().unwrap_or(0) as u32,
                    resultscreen_reader.hits_100().unwrap_or(0) as u32,
                    resultscreen_reader.hits_50().unwrap_or(0) as u32,
                    resultscreen_reader.hits_miss().unwrap_or(0) as u32,
                );
                let total = marv + perf + great + good + bad + miss;

                let accuracy_rating = (310.0 * marv as f64
                    + 300.0 * perf as f64
                    + 200.0 * great as f64
                    + 100.0 * good as f64
                    + 50.0 * bad as f64)
                    / (3.1 * total as f64);
                let player = resultscreen_reader.username().unwrap_or_default();
                // The memory lib can't detect V2
                let accuracy = if mods.bits() & 0x2000_0000 != 0 {
                    (305.0 * marv as f64
                        + 300.0 * perf as f64
                        + 200.0 * great as f64
                        + 100.0 * good as f64
                        + 50.0 * bad as f64)
                        / (3.05 * total as f64)
                } else {
                    resultscreen_reader.accuracy().unwrap_or(0.0)
                };

                println!("Reading beatmap path");
                let mut beatmap_reader =
                    BeatmapReader::new(process, state, OsuClientKind::Stable).unwrap();
                if beatmap_reader.path().is_err() {
                    println!("Read fails, retrying...");
                    return;
                }
                let path = beatmap_reader.path().unwrap();
                println!("Parsing beatmap");
                let mut beatmap = match OsuDataV128::from_file(path.to_str().unwrap()) {
                    Ok(beatmap) => beatmap.to_legacy(),
                    Err(e) => {
                        // Often fails if not mania
                        println!("Error: {e}");
                        return;
                    }
                };

                if beatmap.misc.circle_size != 6 || mods.contains(osu_db::Mod::Random) {
                    // Not 6K
                    *new_score_flag = false;
                    return;
                }

                let info = beatmap.to_beatmap_info(true);
                let md5 = beatmap_reader.md5().unwrap();
                let map_info = RatingMapInfo {
                    hash: md5,
                    path,
                    info,
                };

                let timestamp = Utc::now();
                let play_record = PlayRecord {
                    player,
                    mods: *mods,
                    accuracy,
                    accuracy_rating,
                    judgement_num: [marv, perf, great, good, bad, miss],
                    timestamp,
                };

                println!("Calculating rating");
                if mods.contains(osu_db::Mod::HardRock) || mods.contains(osu_db::Mod::Easy) {
                    let original_od = beatmap.misc.od;
                    let window = 64.5 - (original_od * 3.0).ceil();
                    let new_window = if mods.contains(osu_db::Mod::HardRock) {
                        window / 1.4
                    } else {
                        window * 1.4
                    };
                    let new_od = (64.5 - new_window) / 3.0;
                    beatmap.misc.od = new_od;
                }
                let sr = calculate_from_data(&beatmap, 1.0).unwrap_or(0.0);

                let sr_ht = if mods.contains(osu_db::Mod::HalfTime) {
                    calculate_from_data(&beatmap, 0.75).unwrap_or(0.0)
                } else {
                    0.0
                };

                let sr_dt = if mods.contains(osu_db::Mod::DoubleTime) {
                    calculate_from_data(&beatmap, 1.5).unwrap_or(0.0)
                } else {
                    0.0
                };

                let (diff_const, rating) =
                    calc_mod_rating(*mods, (sr_ht, sr, sr_dt), accuracy_rating);

                let rating_info = RatingInfo {
                    map_info,
                    score_info: play_record,
                    diff_const,
                    rating,
                };

                println!("Updating UI");

                ui_handle
                    .upgrade_in_event_loop(move |ui| {
                        println!("UI Function starts");

                        let realtime_tiles = ui.get_realtime_tiles();
                        let index = realtime_tiles.row_count();
                        let realtime_tiles_vec = realtime_tiles
                            .as_any()
                            .downcast_ref::<VecModel<ScoreTileData>>()
                            .unwrap();
                        let new_pixmap = generate_single_card_pixmap(index, &rating_info).unwrap();
                        let pixel_buffer = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(
                            new_pixmap.data(),
                            new_pixmap.width(),
                            new_pixmap.height(),
                        );
                        let new_data = ScoreTileData {
                            image: Image::from_rgba8(pixel_buffer),
                            index: index as i32,
                        };
                        realtime_tiles_vec.insert(0, new_data);
                        ui.window().request_redraw();
                        println!("UI Function ends");
                    })
                    .unwrap();
                println!("New score added");
            }
            *new_score_flag = false;
        }
        Ok(GameState::Playing) => {
            let mut gameplay_reader = GameplayReader::new(process, state, OsuClientKind::Stable);
            *mods = osu_db::ModSet::from_bits(gameplay_reader.mods().unwrap_or(0));
            *new_score_flag = true;
        }
        _ => {
            *new_score_flag = false;
        }
    }
}
