mod misc;
mod ratings;

pub(crate) use misc::{get_db_path, get_osu_install_path, get_replay_timestamp};
pub use ratings::{prepare_ratings, calc_mod_rating};

use chrono::{DateTime, Utc};
use mania_converter::BeatMapInfo;
use osu_db::ModSet;
use std::path::PathBuf;

// 定义存储结构体
#[derive(Debug, Clone)]
pub struct BeatmapStoreInfo {
    path: PathBuf,
    plays: Vec<PlayRecord>,
}

#[derive(Debug, Clone)]
pub struct PlayRecord {
    pub player: String,
    pub mods: ModSet,
    pub judgement_num: [u32; 6],
    pub accuracy: f64,
    pub accuracy_rating: f64,
    pub timestamp: DateTime<Utc>,
}

impl Default for PlayRecord {
    fn default() -> Self {
        PlayRecord { 
            player: String::new(), 
            mods: ModSet(0), 
            judgement_num: [0,0,0,0,0,0], 
            accuracy: 0.0, 
            accuracy_rating: 0.0, 
            timestamp: DateTime::default() 
        }
    }
}

#[derive(Debug, Clone)]
pub struct RatingMapInfo {
    pub hash: String,
    pub path: PathBuf,
    pub info: BeatMapInfo,
}

#[derive(Debug, Clone)]
pub struct RatingInfo {
    pub map_info: RatingMapInfo,
    pub score_info: PlayRecord,
    pub diff_const: f64,
    pub rating: f64,
}
