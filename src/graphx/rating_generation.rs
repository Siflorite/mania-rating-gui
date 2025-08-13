use chrono::Local;
use handlebars::Handlebars;
use resvg::{tiny_skia, usvg};
use serde_json::json;
use std::{
    env, fs, io,
    path::{Path, PathBuf},
    sync::{Arc, LazyLock},
};

use crate::ScoreTileBase64;
use crate::db::RatingInfo;

const INFO_CARD_TEMPLATE_PATH: &str = "svg/rating_single.svg";
const EXPORT_TEMPLATE_PATH: &str = "svg/export.svg";
const NO_IMAGE_PATH: &str = "svg/no_image.jpg";
const FONT_DIR_PATH: &str = "fonts";
const CARD_HEIGHT: u32 = 350;
const CARD_WIDTH: u32 = 1200;
const PERIMETER: f64 = 628.3185307179586;

const TITLE_MAX_LEN: usize = 28;
const VERSION_MAX_LEN: usize = 55;

static FONT_ARC: LazyLock<Arc<usvg::fontdb::Database>> = LazyLock::new(|| {
    let mut fontdb = usvg::fontdb::Database::new();
    fontdb.load_fonts_dir(FONT_DIR_PATH);
    fontdb.load_system_fonts();
    Arc::new(fontdb)
});

#[derive(serde::Serialize)]
struct CardData {
    bg_image: String,
    title_ascii: String,
    title: String,
    artist_ascii: String,
    artist: String,
    creator: String,
    version: String,
    column_count: u8,
    bpm: String,
    length: String,
    sr_gradient: String,
    sr: String,
    note_str: String,
    ln_str: String,
    len_pos: u32,
    rating_index: u32,
    rating: String,
    diff: String,
    marv_ratio: String,
    marv_extra: Option<String>,
    perf_offset: String,
    perf_ratio: String,
    great_offset: String,
    great_ratio: String,
    good_offset: String,
    good_ratio: String,
    bad_offset: String,
    bad_ratio: String,
    miss_offset: String,
    miss_ratio: String,
    num_marv: u32,
    num_perf: u32,
    num_great: u32,
    num_good: u32,
    num_bad: u32,
    num_miss: u32,
    acc_r: String,
    acc: String,
    diff_mod_color: Option<String>,
    diff_mod_text: String,
    speed_mod_color: Option<String>,
    speed_mod_text: String,
    is_score_v2: bool,
    player_name: String,
    timestamp: String,
    beatmap_hash: String,
    beatmap_url: String,
    status: String,
}

#[derive(serde::Serialize)]
struct ExportCardData {
    x_offset: u32,
    y_offset: u32,
    base64_data: String,
}

fn generate_card_cata(i: usize, info: &RatingInfo) -> CardData {
    let beatmap_info = &info.map_info.info;
    let bg_name = match &beatmap_info.bg_name {
        Some(s) => s.as_str(),
        None => "",
    };
    let osu_file_dir = info.map_info.path.parent().unwrap();
    let bg_path = osu_file_dir.join(Path::new(bg_name));
    let default_path = env::current_dir().unwrap().join(Path::new(NO_IMAGE_PATH));
    let final_path = if bg_path.exists() {
        bg_path
    } else {
        default_path
    };
    let bg_path_string = final_path.to_string_lossy().into_owned().replace("\\", "/");

    let title = beatmap_info
        .title_unicode
        .as_ref()
        .unwrap_or(&beatmap_info.title);
    let title_ascii: &str = if &beatmap_info.title == title {
        ""
    } else {
        &beatmap_info.title
    };
    let title_len = utf8_slice::len(title);
    let title = if title_len > TITLE_MAX_LEN {
        format!(
            "...{}",
            utf8_slice::from(title, title_len + 3 - TITLE_MAX_LEN)
        )
    } else {
        title.clone()
    };
    let artist = beatmap_info
        .artist_unicode
        .as_ref()
        .unwrap_or(&beatmap_info.artist);
    let artist_ascii: &str = if &beatmap_info.artist == artist {
        ""
    } else {
        &beatmap_info.artist
    };
    let version = &beatmap_info.version;
    let version_len = utf8_slice::len(version);
    let version = if version_len > VERSION_MAX_LEN {
        format!("{}...", utf8_slice::till(version, VERSION_MAX_LEN - 3))
    } else {
        version.clone()
    };
    let bpm_str = format_bpm_str(beatmap_info.min_bpm, beatmap_info.max_bpm);
    let delta_len = bpm_str.len() as u32 * 12;
    let length_str = format_length_str(beatmap_info.length);
    let sr = beatmap_info.sr.unwrap_or(0.0);

    let total_count = beatmap_info.note_count + beatmap_info.ln_count;
    let note_str = format!(
        "{} ({:.02}%)",
        beatmap_info.note_count,
        beatmap_info.note_count as f64 / total_count as f64 * 100.0
    );
    let ln_str = format!(
        "{} ({:.02}%) = {}",
        beatmap_info.ln_count,
        beatmap_info.ln_count as f64 / total_count as f64 * 100.0,
        total_count
    );

    let rating_index = i as u32 + 1;
    let rating = format!("{:.02}", info.rating);
    let diff = format!("{:.02}", info.diff_const);

    let num_marv = info.score_info.judgement_num[0];
    let num_perf = info.score_info.judgement_num[1];
    let num_great = info.score_info.judgement_num[2];
    let num_good = info.score_info.judgement_num[3];
    let num_bad = info.score_info.judgement_num[4];
    let num_miss = info.score_info.judgement_num[5];

    let num_total: u32 = info.score_info.judgement_num.iter().sum();
    let marv_ratio = num_marv as f64 / num_total as f64 * PERIMETER;
    let perf_offset = marv_ratio;
    let perf_ratio = num_perf as f64 / num_total as f64 * PERIMETER;
    let great_offset = perf_offset + perf_ratio;
    let great_ratio = num_great as f64 / num_total as f64 * PERIMETER;
    let good_offset = great_offset + great_ratio;
    let good_ratio = num_good as f64 / num_total as f64 * PERIMETER;
    let bad_offset = good_offset + good_ratio;
    let bad_ratio = num_bad as f64 / num_total as f64 * PERIMETER;
    let miss_offset = bad_offset + bad_ratio;
    let miss_ratio = num_miss as f64 / num_total as f64 * PERIMETER;
    let (marv_ratio, marv_extra) = (marv_ratio.min(20.04), (marv_ratio - 20.04).max(0.0));

    let acc_r = format!("{:.02}", info.score_info.accuracy_rating);
    let acc = format!("{:.02}", info.score_info.accuracy);

    let beatmap_hash = info.map_info.hash.clone();
    let (beatmap_url, status) = if beatmap_info.beatmap_set_id == -1 || beatmap_info.beatmap_id == 0
    {
        (String::new(), "Unsubmitted")
    } else {
        // 需要有一个方法验证beatmap在官网的状态，暂时无法实现
        (
            format!(
                "/beatmapsets/{}#mania/{}",
                beatmap_info.beatmap_set_id, beatmap_info.beatmap_id
            ),
            "",
        )
    };

    let mods = info.score_info.mods;
    let is_score_v2 = mods.bits() & 0x2000_0000 != 0;
    enum SpeedMod {
        NC,
        DT,
        HT,
        NM,
    }
    let speed_mod = if mods.contains(osu_db::Mod::Nightcore) {
        SpeedMod::NC
    } else if mods.contains(osu_db::Mod::DoubleTime) {
        SpeedMod::DT
    } else if mods.contains(osu_db::Mod::HalfTime) {
        SpeedMod::HT
    } else {
        SpeedMod::NM
    };
    let speed_mod_color = match speed_mod {
        SpeedMod::NC | SpeedMod::DT => Some("purple".into()),
        SpeedMod::HT => Some("gray".into()),
        SpeedMod::NM => None,
    };
    let speed_mod_text = match speed_mod {
        SpeedMod::NC => "NC".into(),
        SpeedMod::DT => "DT".into(),
        SpeedMod::HT => "HT".into(),
        SpeedMod::NM => "".into(),
    };

    enum DiffMod {
        HR,
        EZ,
        NM,
    }
    let diff_mod = if mods.contains(osu_db::Mod::HardRock) {
        DiffMod::HR
    } else if mods.contains(osu_db::Mod::Easy) {
        DiffMod::EZ
    } else {
        DiffMod::NM
    };
    let diff_mod_color = match diff_mod {
        DiffMod::HR => Some("red".into()),
        DiffMod::EZ => Some("green".into()),
        DiffMod::NM => None,
    };
    let diff_mod_text = match diff_mod {
        DiffMod::HR => "HR".into(),
        DiffMod::EZ => "EZ".into(),
        DiffMod::NM => "".into(),
    };

    CardData {
        bg_image: bg_path_string,
        title_ascii: title_ascii.into(),
        title,
        artist_ascii: artist_ascii.into(),
        artist: artist.into(),
        creator: beatmap_info.creator.clone(),
        version,
        column_count: beatmap_info.column_count,
        bpm: bpm_str,
        length: length_str,
        sr_gradient: format_sr_gradient(sr),
        sr: format!("{sr:.02}"),
        note_str,
        ln_str,
        len_pos: 150 + delta_len,
        rating_index,
        rating,
        diff,
        marv_ratio: format!("{marv_ratio:.02}"),
        marv_extra: if marv_extra > 0.0 {
            Some(format!("{marv_extra:.02}"))
        } else {
            None
        },
        perf_offset: format!("{perf_offset:.02}"),
        perf_ratio: format!("{perf_ratio:.02}"),
        great_offset: format!("{great_offset:.02}"),
        great_ratio: format!("{great_ratio:.02}"),
        good_offset: format!("{good_offset:.02}"),
        good_ratio: format!("{good_ratio:.02}"),
        bad_offset: format!("{bad_offset:.02}"),
        bad_ratio: format!("{bad_ratio:.02}"),
        miss_offset: format!("{miss_offset:.02}"),
        miss_ratio: format!("{miss_ratio:.02}"),
        num_marv,
        num_perf,
        num_great,
        num_good,
        num_bad,
        num_miss,
        acc_r,
        acc,
        diff_mod_color,
        diff_mod_text,
        speed_mod_color,
        speed_mod_text,
        is_score_v2,
        player_name: info.score_info.player.clone(),
        timestamp: info
            .score_info
            .timestamp
            .with_timezone(&Local)
            .format("%Y-%m-%d %H:%M:%S")
            .to_string(),
        beatmap_hash,
        beatmap_url,
        status: status.into(),
    }
}

fn generate_export_data(info_vec: Vec<ScoreTileBase64>) -> Vec<ExportCardData> {
    info_vec
        .into_iter()
        .enumerate()
        .map(|(i, info)| ExportCardData {
            x_offset: i as u32 % 3 * CARD_WIDTH,
            y_offset: (i as u32 / 3 + 1) * CARD_HEIGHT,
            base64_data: info.base64_string,
        })
        .collect()
}

pub fn export_info(
    player_name: &str,
    info_vec: Vec<ScoreTileBase64>,
    average_rating: f64,
) -> io::Result<PathBuf> {
    let y_disclaimer = ((info_vec.len() as f64 / 3.0).ceil() as u32 + 1) * CARD_HEIGHT;
    let total_height = y_disclaimer + 150;
    let player_name_f = if player_name == "[Recent 30]" {
        "Recent 30".into()
    } else {
        format!("Player: {player_name}")
    };
    let average_rating_fill = format_diff_gradient(average_rating);
    let average_rating = format!("{average_rating:.02}");
    let generated_time = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let cards = generate_export_data(info_vec);

    let mut reg = Handlebars::new();
    reg.register_template_file("template", EXPORT_TEMPLATE_PATH)
        .expect("Failed to register template");
    let svg_content = reg
        .render(
            "template",
            &json!({
                "total_height": total_height,
                "player_name": player_name_f,
                "average_rating_fill": average_rating_fill,
                "average_rating": average_rating,
                "generated_time": generated_time,
                "cards": cards,
                "y_disclaimer": y_disclaimer
            }),
        )
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    let options = usvg::Options {
        fontdb: FONT_ARC.clone(),
        ..Default::default()
    };

    // 解析并渲染SVG
    let tree = usvg::Tree::from_str(&svg_content, &options)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    let mut pixmap = tiny_skia::Pixmap::new(3600, total_height)
        .ok_or_else(|| io::Error::other("Failed to create pixmap"))?;

    resvg::render(&tree, tiny_skia::Transform::default(), &mut pixmap.as_mut());

    // 确保输出目录存在
    let save_pic_path = env::current_dir().unwrap().join("export");
    if let Some(parent) = save_pic_path.parent() {
        fs::create_dir_all(parent)?;
    }

    // 保存为PNG
    // let santized_name = sanitize_filename(&info_vec[0].score_info.player);
    let pic_name = format!("{player_name}.jpg");

    if !save_pic_path.exists() {
        fs::create_dir_all(&save_pic_path)?;
    }
    let pic_path = save_pic_path.join(pic_name);

    let image = image::RgbaImage::from_raw(pixmap.width(), pixmap.height(), pixmap.take()).unwrap();

    // Rgba8不支持导出到Jpeg
    let rgb_image = image::DynamicImage::ImageRgba8(image).to_rgb8();
    rgb_image
        .save_with_format(&pic_path, image::ImageFormat::Jpeg)
        .map_err(io::Error::other)?;

    // Too slow!
    // let mut output_file = fs::File::create(&pic_path)?;
    // let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut output_file, 80);
    // encoder.encode_image(&rgb_image).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

    Ok(pic_path)
}

pub fn generate_single_card_pixmap(i: usize, info: &RatingInfo) -> io::Result<tiny_skia::Pixmap> {
    let card_data = generate_card_cata(i, info);
    let mut reg = Handlebars::new();
    reg.register_template_file("template", INFO_CARD_TEMPLATE_PATH)
        .expect("Failed to register template");
    let svg_content: String = reg
        .render("template", &json!(card_data))
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    let options = usvg::Options {
        fontdb: FONT_ARC.clone(),
        ..Default::default()
    };

    let tree = usvg::Tree::from_str(&svg_content, &options)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    let mut pixmap = tiny_skia::Pixmap::new(CARD_WIDTH, CARD_HEIGHT)
        .ok_or_else(|| io::Error::other("Failed to create pixmap"))?;

    resvg::render(&tree, tiny_skia::Transform::default(), &mut pixmap.as_mut());

    Ok(pixmap)
}

fn format_bpm_str(min_bpm: f64, max_bpm: Option<f64>) -> String {
    let m_bpm = match max_bpm {
        Some(v) => v,
        None => min_bpm,
    };
    let min_bpm_str = format!("{min_bpm:.1}")
        .trim_matches('0')
        .trim_matches('.')
        .to_string();

    if (m_bpm * 10.0).round() as i32 == (min_bpm * 10.0).round() as i32 {
        min_bpm_str.to_string()
    } else {
        let max_bpm_str = format!("{m_bpm:.1}")
            .trim_matches('0')
            .trim_matches('.')
            .to_string();
        format!("{min_bpm_str}-{max_bpm_str}")
    }
}

fn format_length_str(length: u32) -> String {
    let mins = length / 60000;
    let secs = (length - 60000 * mins) / 1000;
    // let msecs = length % 1000;
    format!("{mins}:{secs:02}")
}

fn format_sr_gradient(sr: f64) -> String {
    let colors = [
        (79.0, 192.0, 255.0),
        (124.0, 255.0, 79.0),
        (246.0, 240.0, 92.0),
        (255.0, 78.0, 111.0),
        (198.0, 69.0, 184.0),
        (101.0, 99.0, 222.0),
        (0.0, 0.0, 0.0),
        (0.0, 0.0, 0.0),
    ];
    let sr = sr.clamp(0.0, 10.0);
    let interval = 10.0 / (colors.len() - 2) as f64;
    let section = (sr / interval) as usize;
    let partial = (sr - interval * section as f64) / interval;
    let r = colors[section].0 + (colors[section + 1].0 - colors[section].0) * partial;
    let g = colors[section].1 + (colors[section + 1].1 - colors[section].1) * partial;
    let b = colors[section].2 + (colors[section + 1].2 - colors[section].2) * partial;
    format!(
        "rgb({},{},{})",
        r.round() as u8,
        g.round() as u8,
        b.round() as u8
    )
}

fn format_diff_gradient(diff: f64) -> String {
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
    // (r as u8, g as u8, b as u8)
    format!(
        "rgb({},{},{})",
        r.round() as u8,
        g.round() as u8,
        b.round() as u8
    )
}
