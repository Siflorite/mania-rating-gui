use chrono::{DateTime, TimeZone, Utc};
use std::path::PathBuf;
use winreg::RegKey;
use winreg::enums::*;

pub fn get_osu_install_path() -> Option<PathBuf> {
    let try_registry = |key: *mut core::ffi::c_void, path: &str| -> Option<PathBuf> {
        RegKey::predef(key)
            .open_subkey(path)
            .ok()?
            .get_value::<String, &str>("")
            .ok()?
            .split('\"')
            .nth(1)
            .and_then(|exe_path| PathBuf::from(exe_path).parent().map(|p| p.to_path_buf()))
            .filter(|p| p.is_dir())
    };

    try_registry(HKEY_CLASSES_ROOT, r"osustable.File.osz\Shell\Open\Command").or_else(|| {
        try_registry(
            HKEY_CURRENT_USER,
            r"Software\Classes\osustable.File.osz\Shell\Open\Command",
        )
    })
}

pub(crate) fn get_db_path(osu_exe_dir: &str, db: &str) -> Option<PathBuf> {
    let osu_exe_dir = if osu_exe_dir.is_empty() {
        get_osu_install_path()?
    } else {
        PathBuf::from(osu_exe_dir)
    };

    let scores_db_path = osu_exe_dir.join(db);
    if scores_db_path.exists() {
        Some(scores_db_path)
    } else {
        None
    }
}

#[allow(dead_code)]
pub(crate) fn get_replay_file_name(timestamp: DateTime<Utc>, hash: &str) -> String {
    let delta_u64 = get_replay_timestamp(timestamp);
    format!("{hash}-{delta_u64}.osr")
}

#[allow(dead_code)]
pub(crate) fn get_replay_timestamp(timestamp: DateTime<Utc>) -> u64 {
    let start_epoch = Utc.with_ymd_and_hms(1601, 1, 1, 0, 0, 0).unwrap();
    let delta = timestamp.signed_duration_since(start_epoch);
    let days_delta = delta.num_days();
    let days_100ns = days_delta as u64 * 24 * 60 * 60 * 10_000_000;
    let time_delta = timestamp.time().signed_duration_since(start_epoch.time());
    let time_100ns = time_delta.num_nanoseconds().unwrap() as u64 / 100;
    days_100ns + time_100ns
}
