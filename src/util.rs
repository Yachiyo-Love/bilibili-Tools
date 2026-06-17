use std::{
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use tokio::fs;

use crate::models::{PageData, ViewData};

pub async fn unique_path(path: PathBuf) -> PathBuf {
    if fs::metadata(&path).await.is_err() {
        return path;
    }

    let parent = path.parent().map(Path::to_path_buf).unwrap_or_default();
    let stem = path
        .file_stem()
        .and_then(|part| part.to_str())
        .unwrap_or("download");
    let extension = path.extension().and_then(|part| part.to_str()).unwrap_or("");

    for index in 1.. {
        let filename = if extension.is_empty() {
            format!("{stem} ({index})")
        } else {
            format!("{stem} ({index}).{extension}")
        };
        let candidate = parent.join(filename);
        if fs::metadata(&candidate).await.is_err() {
            return candidate;
        }
    }

    unreachable!("infinite loop returns a path")
}

pub fn make_output_stem(view: &ViewData, page: &PageData) -> String {
    let raw = if view.pages.len() > 1 {
        format!("{} - P{} {}", view.title, page.page, page.part)
    } else {
        view.title.clone()
    };

    let safe = sanitize_filename(&raw);
    if safe.is_empty() {
        format!("BV{}", view.aid)
    } else {
        truncate_chars(&safe, 120)
    }
}

pub fn sanitize_filename(name: &str) -> String {
    let mut output = String::with_capacity(name.len());
    for ch in name.chars() {
        let is_invalid = matches!(ch, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*')
            || ch.is_control();
        output.push(if is_invalid { '_' } else { ch });
    }

    output.trim_matches([' ', '.']).to_owned()
}

pub fn truncate_chars(text: &str, max_chars: usize) -> String {
    text.chars().take(max_chars).collect()
}

pub fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub fn path_arg(path: &Path) -> &str {
    path.to_str()
        .expect("This tool requires paths that are valid UTF-8")
}

pub fn quality_label(qn: u32) -> String {
    match qn {
        120 => "4K".to_owned(),
        116 => "1080P60".to_owned(),
        112 => "1080P+".to_owned(),
        80 => "1080P".to_owned(),
        74 => "720P60".to_owned(),
        64 => "720P".to_owned(),
        32 => "480P".to_owned(),
        16 => "360P".to_owned(),
        other => format!("QN{other}"),
    }
}

pub fn format_accept_quality(qualities: &[u32], descriptions: &[String]) -> String {
    qualities
        .iter()
        .enumerate()
        .map(|(index, quality)| {
            descriptions
                .get(index)
                .cloned()
                .unwrap_or_else(|| quality_label(*quality))
        })
        .collect::<Vec<_>>()
        .join(", ")
}
