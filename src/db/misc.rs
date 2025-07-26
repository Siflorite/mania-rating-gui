use chrono::{DateTime, TimeZone, Utc};
use std::io;
use std::path::PathBuf;
use winreg::RegKey;
use winreg::enums::*;

pub(crate) fn get_osu_install_path() -> io::Result<PathBuf> {
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

    try_registry(HKEY_CLASSES_ROOT, r"osustable.File.osz\Shell\Open\Command")
        .or_else(|| {
            try_registry(
                HKEY_CURRENT_USER,
                r"Software\Classes\osustable.File.osz\Shell\Open\Command",
            )
        })
        .ok_or(io::Error::new(
            io::ErrorKind::NotFound,
            "Could not find osu path",
        ))
}

pub(crate) fn get_db_path(osu_exe_dir: &str, db: &str) -> io::Result<PathBuf> {
    let osu_exe_dir = if osu_exe_dir.is_empty() {
        get_osu_install_path()?
    } else {
        PathBuf::from(osu_exe_dir)
    };

    let scores_db_path = osu_exe_dir.join(db);
    if scores_db_path.exists() {
        return Ok(scores_db_path);
    } else {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("Could not find {db}"),
        ));
    }
}

#[allow(dead_code)]
pub(crate) fn get_replay_file_name(timestamp: DateTime<Utc>, hash: &str) -> String {
    let start_epoch = Utc.with_ymd_and_hms(1601, 1, 1, 0, 0, 0).unwrap();
    let delta = timestamp.signed_duration_since(start_epoch);
    let days_delta = delta.num_days();
    let days_100ns = days_delta as u64 * 24 * 60 * 60 * 10_000_000;
    let time_delta = timestamp.time().signed_duration_since(start_epoch.time());
    let time_100ns = time_delta.num_nanoseconds().unwrap() as u64 / 100;
    let delta_u64 = days_100ns + time_100ns;
    format!("{}-{}.osr", hash, delta_u64)
}

#[allow(dead_code)]
pub(crate) fn format_diff_gradient(diff: f64) -> (u8, u8, u8) {
    let colors = [
        (79.0, 192.0, 255.0),
        (124.0, 255.0, 79.0),
        (246.0, 240.0, 92.0),
        (255.0, 78.0, 111.0),
        (198.0, 69.0, 184.0),
        (101.0, 99.0, 222.0),
        (0.0, 0.0, 0.0),
    ];
    let diff_section = [0.0, 7.0, 12.0, 16.0, 19.0, 22.0, 25.0];

    let diff = diff.clamp(0.0, 25.0);
    let section = diff_section
        .partition_point(|&x| x < diff)
        .min(colors.len() - 2);
    let interval = diff_section[section + 1] - diff_section[section];
    let partial = (diff - diff_section[section]) / interval;
    let r = colors[section].0 + (colors[section + 1].0 - colors[section].0) * partial;
    let g = colors[section].1 + (colors[section + 1].1 - colors[section].1) * partial;
    let b = colors[section].2 + (colors[section + 1].2 - colors[section].2) * partial;
    (r as u8, g as u8, b as u8)
}