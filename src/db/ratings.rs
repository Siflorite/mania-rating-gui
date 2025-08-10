use chrono::{DateTime, Utc};
// use chrono::Local;
// use colored::Colorize;
// use indicatif::{ParallelProgressIterator, ProgressBar, ProgressStyle};
use osu_db::{self, Listing, Mod, ModSet, ScoreList, Replay};
use rayon::prelude::*;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::io;
use std::path::PathBuf;
use walkdir::WalkDir;

use crate::db::{BeatmapStoreInfo, PlayRecord, RatingInfo};
use crate::db::{get_db_path, get_osu_install_path, get_replay_timestamp};
use mania_converter::osu_func::{OsuDataV128, calculate_from_data};

use super::RatingMapInfo;

pub fn extract_plays(osu_exe_dir: &str) -> io::Result<HashMap<String, BeatmapStoreInfo>> {
    // 读取谱面数据库
    let osu_path = if osu_exe_dir.is_empty() {
        get_osu_install_path().ok_or(io::Error::new(io::ErrorKind::InvalidData, "cannot find osu!.exe"))?
    } else {
        PathBuf::from(osu_exe_dir)
    };
    println!("osu!.exe所在文件夹路径：{:?}", osu_path);
    let listing = Listing::from_file(get_db_path(osu_exe_dir, "osu!.db")
        .ok_or(io::Error::new(io::ErrorKind::InvalidData, "cannot find osu!.exe"))?)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let scores = ScoreList::from_file(get_db_path(osu_exe_dir, "scores.db")
        .ok_or(io::Error::new(io::ErrorKind::InvalidData, "cannot find osu!.exe"))?)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    // 构建哈希映射存储谱面信息
    let mut beatmap_store: HashMap<String, BeatmapStoreInfo> = HashMap::new();

    for b in listing.beatmaps {
        if b.circle_size == 6.0
            && b.mode == osu_db::Mode::Mania
            && b.folder_name.is_some()
            && b.file_name.is_some()
            && b.hash.is_some()
        {
            let path = osu_path
                .join("Songs")
                .join(b.folder_name.unwrap())
                .join(b.file_name.unwrap());

            let info = BeatmapStoreInfo {
                path,
                plays: Vec::new(),
            };

            beatmap_store.insert(b.hash.unwrap(), info);
        }
    }

    // 处理每一条游玩记录
    for score in scores.beatmaps {
        for rep in score.scores {
            if rep.mode == osu_db::Mode::Mania
                && rep.beatmap_hash.is_some()
                && rep.player_name.is_some()
            {
                // 计算准确度
                let total = rep.count_geki
                    + rep.count_300
                    + rep.count_katsu
                    + rep.count_100
                    + rep.count_50
                    + rep.count_miss;
                // https://osu.ppy.sh/wiki/en/Client/File_formats/osr_%28file_format%29
                // According to osu, 29 stands for ScoreV2, and 30 stands for Mirror
                let (accuracy, accuracy_rating) = if rep.mods.bits() & 0x2000_0000 != 0 {
                    // ScoreV2
                    let acc = (305.0 * rep.count_geki as f64
                        + 300.0 * rep.count_300 as f64
                        + 200.0 * rep.count_katsu as f64
                        + 100.0 * rep.count_100 as f64
                        + 50.0 * rep.count_50 as f64)
                        / (3.05 * total as f64);
                    let acc_r = (310.0 * rep.count_geki as f64
                        + 300.0 * rep.count_300 as f64
                        + 200.0 * rep.count_katsu as f64
                        + 100.0 * rep.count_100 as f64
                        + 50.0 * rep.count_50 as f64)
                        / (3.1 * total as f64);
                    (acc, acc_r)
                } else {
                    let acc = (300.0 * (rep.count_geki + rep.count_300) as f64
                        + 200.0 * rep.count_katsu as f64
                        + 100.0 * rep.count_100 as f64
                        + 50.0 * rep.count_50 as f64)
                        / (3.0 * total as f64);
                    let acc_r = (310.0 * rep.count_geki as f64
                        + 300.0 * rep.count_300 as f64
                        + 200.0 * rep.count_katsu as f64
                        + 100.0 * rep.count_100 as f64
                        + 50.0 * rep.count_50 as f64)
                        / (3.1 * total as f64);
                    (acc, acc_r)
                };

                // let replay_file_name = get_replay_file_name(rep.timestamp, &rep.beatmap_hash.clone().unwrap());
                // let replay_path = osu_path.join("Data").join("r").join(replay_file_name);
                // if replay_path.is_file() {
                //     if let Ok(rep) = Replay::from_file(replay_path) {
                //         println!("{:?}", rep);
                //     }
                // }

                // 更新对应谱面的记录
                if let Some(info) = beatmap_store.get_mut(&rep.beatmap_hash.unwrap()) {
                    let judgement_vec = [
                        rep.count_geki as u32,
                        rep.count_300 as u32,
                        rep.count_katsu as u32,
                        rep.count_100 as u32,
                        rep.count_50 as u32,
                        rep.count_miss as u32,
                    ];
                    info.plays.push(PlayRecord {
                        player: rep.player_name.unwrap(),
                        mods: rep.mods.clone(),
                        judgement_num: judgement_vec,
                        accuracy,
                        accuracy_rating,
                        timestamp: rep.timestamp,
                    });
                }
            }
        }
    }

    Ok(beatmap_store)
}

/// 从 osu目录/Data/r 目录下面找到比scores.db中更新的但还未写入的回放，计入信息中
/// 流程：遍历回放目录的文件名，筛选时间戳大于scores.db中最新时间戳的osr文件，记录其中信息
#[allow(dead_code)]
pub fn extract_unstored_replays(osu_exe_dir: &str, timestamp: DateTime<Utc>) -> io::Result<Vec<PlayRecord>> {
    let osu_path = if osu_exe_dir.is_empty() {
        get_osu_install_path().ok_or(io::Error::new(io::ErrorKind::InvalidData, "cannot find osu!.exe"))?
    } else {
        PathBuf::from(osu_exe_dir)
    };

    let timestamp = get_replay_timestamp(timestamp);

    let replay_dir = osu_path.join("Data").join("r");
    let record_vec: Vec<PlayRecord> = WalkDir::new(&replay_dir)
        .into_iter()
        .filter_map(|entry| entry.ok()) 
        .filter(|entry| {
            let path = entry.path();
            let (file_stem, ext) = (path.file_stem(), path.extension().unwrap().to_str());
            let timestamp_osr = file_stem.unwrap().to_str().unwrap_or("")
                .split("-").last().unwrap_or("").parse::<u64>().unwrap_or(0);
            timestamp_osr > timestamp && ext == Some("osr")
        })
        .filter_map(|entry| {
            let path = entry.path();
            let rep = Replay::from_file(path).ok()?;
            if rep.mode == osu_db::Mode::Mania {
                // 计算准确度
                let total = rep.count_geki
                    + rep.count_300
                    + rep.count_katsu
                    + rep.count_100
                    + rep.count_50
                    + rep.count_miss;
                // https://osu.ppy.sh/wiki/en/Client/File_formats/osr_%28file_format%29
                // According to osu, 29 stands for ScoreV2, and 30 stands for Mirror
                let (accuracy, accuracy_rating) = if rep.mods.bits() & 0x2000_0000 != 0 {
                    // ScoreV2
                    let acc = (305.0 * rep.count_geki as f64
                        + 300.0 * rep.count_300 as f64
                        + 200.0 * rep.count_katsu as f64
                        + 100.0 * rep.count_100 as f64
                        + 50.0 * rep.count_50 as f64)
                        / (3.05 * total as f64);
                    let acc_r = (310.0 * rep.count_geki as f64
                        + 300.0 * rep.count_300 as f64
                        + 200.0 * rep.count_katsu as f64
                        + 100.0 * rep.count_100 as f64
                        + 50.0 * rep.count_50 as f64)
                        / (3.1 * total as f64);
                    (acc, acc_r)
                } else {
                    let acc = (300.0 * (rep.count_geki + rep.count_300) as f64
                        + 200.0 * rep.count_katsu as f64
                        + 100.0 * rep.count_100 as f64
                        + 50.0 * rep.count_50 as f64)
                        / (3.0 * total as f64);
                    let acc_r = (310.0 * rep.count_geki as f64
                        + 300.0 * rep.count_300 as f64
                        + 200.0 * rep.count_katsu as f64
                        + 100.0 * rep.count_100 as f64
                        + 50.0 * rep.count_50 as f64)
                        / (3.1 * total as f64);
                    (acc, acc_r)
                };

                Some(PlayRecord {
                    player: rep.player_name?,
                    mods: rep.mods,
                    judgement_num: [
                        rep.count_geki as u32,
                        rep.count_300 as u32,
                        rep.count_katsu as u32,
                        rep.count_100 as u32,
                        rep.count_50 as u32,
                        rep.count_miss as u32,
                    ],
                    accuracy: accuracy,
                    accuracy_rating: accuracy_rating,
                    timestamp: rep.timestamp,
                })
            } else {
                None
            }
        }).collect();
    Ok(record_vec)
}

#[inline]
fn calc_rating(diff_const: f64, acc: f64) -> f64 {
    if acc < 0.0 || acc > 100.0 {
        return 0.0;
    }

    let diff_lower = (diff_const - 3.0).max(0.0);
    if acc <= 80.0 {
        0.0
    } else if acc <= 93.0 {
        diff_lower * (acc - 80.0) / 13.0
    } else if acc <= 96.0 {
        (diff_const - diff_lower) * (acc - 93.0) / 3.0 + diff_lower
    } else if acc <= 98.0 {
        let acc_xtra = acc - 96.0;
        1.5 * acc_xtra / (3.0 - acc_xtra / 2.0) + diff_const
    } else if acc <= 99.5 {
        let acc_xtra2 = (acc - 98.0) / 1.5;
        2.0 * acc_xtra2 * 2.0 / (3.0 - acc_xtra2) + diff_const + 1.5
    } else {
        let acc_xtra3 = (acc - 99.5) * 2.0;
        acc_xtra3 * 2.0 / (3.0 - acc_xtra3) + diff_const + 3.5
    }
}

#[inline]
pub fn calc_mod_rating(mods: ModSet, srs: (f64, f64, f64), acc: f64) -> (f64, f64) {
    let sr_mod = if mods.contains(Mod::HalfTime) {
        srs.0
    } else if mods.contains(Mod::DoubleTime) || mods.contains(Mod::Nightcore) {
        srs.2
    } else {
        srs.1
    };
    let diff_const = sr_mod * 200.0 / 81.0 + 7.0 / 6.0;
    let rating = calc_rating(diff_const, acc);
    (diff_const, rating)
}

pub fn extract_ratings(osu_exe_dir: &str) -> io::Result<(Vec<RatingInfo>, Vec<RatingInfo>)> {
    // 读取谱面数据库
    let beatmap_store: Vec<(String, BeatmapStoreInfo)> = extract_plays(osu_exe_dir)?
        .into_par_iter()
        .filter_map(|(hash, mut info)| {
            info.plays.retain(|p| !p.mods.contains(Mod::Random));
            if info.plays.is_empty() {
                return None;
            }
            Some((hash, info))
        })
        .collect();

    let processed: Vec<(Vec<RatingInfo>, usize)> = beatmap_store
        .into_par_iter()
        // .progress_with(pb)
        // .with_style(style)
        .filter_map(|(hash, info)| {
            // println!("{:?}", info.path);
            let osu_data = OsuDataV128::from_file(&info.path.to_str()?)
                .ok()?
                .to_legacy();
            let beatmap_info = osu_data.to_beatmap_info(true);

            let mut mod_flag = (false, false, false);
            for p in info.plays.iter() {
                if p.mods.contains(Mod::HalfTime) {
                    mod_flag.0 = true;
                } else if p.mods.contains(Mod::DoubleTime) || p.mods.contains(Mod::Nightcore) {
                    mod_flag.2 = true;
                } else {
                    mod_flag.1 = true;
                };
            }

            let sr = beatmap_info.sr.unwrap_or(0.0);
            let (sr_ht, sr_dt) = (
                if mod_flag.0 {
                    calculate_from_data(&osu_data, 0.75).ok()?
                } else {
                    0.0
                },
                if mod_flag.2 {
                    calculate_from_data(&osu_data, 1.5).ok()?
                } else {
                    0.0
                },
            );
            let srs = (sr_ht, sr, sr_dt);

            // 生成所有play记录
            let all_plays: Vec<RatingInfo> = info
                .plays
                .iter()
                .map(|play| {
                    let (diff_const, rating) =
                        calc_mod_rating(play.mods, srs, play.accuracy_rating);
                    RatingInfo {
                        map_info: RatingMapInfo {
                            hash: hash.clone(),
                            path: info.path.clone(),
                            info: beatmap_info.clone(),
                        },
                        score_info: play.clone(),
                        diff_const,
                        rating,
                    }
                })
                .collect();

            let best_play_index = all_plays
                .iter()
                .enumerate()
                .max_by(|(_, a), (_, b)| a.rating.partial_cmp(&b.rating).unwrap_or(Ordering::Equal))
                .map(|(i, _)| i)?;

            Some((all_plays, best_play_index))
        })
        .collect();

    let all_ratings: Vec<_> = processed
        .iter()
        .flat_map(|(plays, _)| plays.iter().cloned())
        .collect();
    let best_ratings: Vec<_> = processed
        .into_iter()
        .map(|(plays, i)| plays[i].clone())
        .collect();

    Ok((all_ratings, best_ratings))
}

pub fn prepare_ratings(osu_exe_dir: &str) -> io::Result<HashMap<String, Vec<RatingInfo>>> {
    // 读取谱面数据库
    let (mut all_ratings, mut best_ratings) = extract_ratings(osu_exe_dir)?;

    // Recent Scores
    all_ratings.sort_unstable_by(|a, b| {
        b.score_info.timestamp
            .partial_cmp(&a.score_info.timestamp)
            .unwrap_or(Ordering::Equal)
    });
    let all_ratings_clone = all_ratings.clone();

    best_ratings.sort_unstable_by(|a, b| {
        b.rating.partial_cmp(&a.rating).unwrap_or(Ordering::Equal)
    });

    // 玩家列表
    let players_vec = {
        let mut players: HashSet<String> = HashSet::new();
        for record in all_ratings.iter() {
            players.insert(record.score_info.player.clone());
        }
        players.into_iter().collect::<Vec<_>>()
    };

    // 对每个玩家，生成Rating列表，对于谱面Hash相同的选择最高分数
    let mut player_scores = players_vec
        .into_iter()
        .map(|player| {
            let player_ratings_origin = all_ratings.extract_if(.., |rating| {
                rating.score_info.player == player
            }).collect::<Vec<_>>();
            
            let mut hash_ratings: HashMap<String, RatingInfo> = HashMap::new();
            player_ratings_origin.iter().for_each(|info| {
                let entry = hash_ratings.entry(info.map_info.hash.clone())
                    .or_insert(info.clone());
                if info.rating > entry.rating {
                    *entry = info.clone();
                }
            });

            let mut player_ratings_final = hash_ratings.into_iter()
                .map(|(_, info)| info)
                .collect::<Vec<_>>();
            player_ratings_final.sort_unstable_by(|a, b| {
                b.rating.partial_cmp(&a.rating).unwrap_or(Ordering::Equal)
            });

            let final_name = match player.as_str() {
                "Recent 30" | "All Players" => format!("{} (Player)", player),
                _ => player,
            };

            (final_name, player_ratings_final)

        }).collect::<HashMap<_, _>>();


    player_scores.insert("[Recent 30]".into(), all_ratings_clone);
    player_scores.insert("[All Players]".into(), best_ratings);

    Ok(player_scores)
}