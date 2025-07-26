mod misc;
mod ratings;

pub(crate) use misc::{get_db_path, get_osu_install_path};
pub use ratings::prepare_ratings;

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
