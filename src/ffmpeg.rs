use std::{
    env,
    fs::File as StdFile,
    path::{Path, PathBuf},
    process::Stdio,
};

use anyhow::{bail, Context, Result};
use futures_util::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::{header::REFERER, Client};
use tokio::{
    fs::{self, File},
    io::AsyncWriteExt,
    process::Command,
};
use zip::ZipArchive;

use crate::{
    api::build_client,
    constants::{FFMPEG_BUILD_URL, ROOT_REFERER},
    util::path_arg,
};

pub async fn mux_mp4(
    ffmpeg: &Path,
    video_path: &Path,
    audio_path: &Path,
    output_path: &Path,
) -> Result<()> {
    run_ffmpeg(
        ffmpeg,
        [
            "-y",
            "-hide_banner",
            "-loglevel",
            "error",
            "-i",
            path_arg(video_path),
            "-i",
            path_arg(audio_path),
            "-c",
            "copy",
            path_arg(output_path),
        ],
        "合并 MP4 失败",
    )
    .await
}

pub async fn remux_single(ffmpeg: &Path, input_path: &Path, output_path: &Path) -> Result<()> {
    run_ffmpeg(
        ffmpeg,
        [
            "-y",
            "-hide_banner",
            "-loglevel",
            "error",
            "-i",
            path_arg(input_path),
            "-c",
            "copy",
            path_arg(output_path),
        ],
        "重新封装 MP4 失败",
    )
    .await
}

pub async fn transcode_mp3(ffmpeg: &Path, input_path: &Path, output_path: &Path) -> Result<()> {
    run_ffmpeg(
        ffmpeg,
        [
            "-y",
            "-hide_banner",
            "-loglevel",
            "error",
            "-i",
            path_arg(input_path),
            "-vn",
            "-codec:a",
            "libmp3lame",
            "-q:a",
            "2",
            path_arg(output_path),
        ],
        "转码 MP3 失败",
    )
    .await
}

pub async fn remux_audio_m4a(ffmpeg: &Path, input_path: &Path, output_path: &Path) -> Result<()> {
    run_ffmpeg(
        ffmpeg,
        [
            "-y",
            "-hide_banner",
            "-loglevel",
            "error",
            "-i",
            path_arg(input_path),
            "-vn",
            "-c:a",
            "copy",
            "-movflags",
            "+faststart",
            path_arg(output_path),
        ],
        "重封装音频为 M4A 失败",
    )
    .await
}

pub async fn remux_concat(ffmpeg: &Path, list_path: &Path, output_path: &Path) -> Result<()> {
    run_ffmpeg(
        ffmpeg,
        [
            "-y",
            "-hide_banner",
            "-loglevel",
            "error",
            "-f",
            "concat",
            "-safe",
            "0",
            "-i",
            path_arg(list_path),
            "-c",
            "copy",
            path_arg(output_path),
        ],
        "拼接 MP4 失败",
    )
    .await
}

pub async fn transcode_concat_mp3(
    ffmpeg: &Path,
    list_path: &Path,
    output_path: &Path,
) -> Result<()> {
    run_ffmpeg(
        ffmpeg,
        [
            "-y",
            "-hide_banner",
            "-loglevel",
            "error",
            "-f",
            "concat",
            "-safe",
            "0",
            "-i",
            path_arg(list_path),
            "-vn",
            "-codec:a",
            "libmp3lame",
            "-q:a",
            "2",
            path_arg(output_path),
        ],
        "转码拼接后的 MP3 失败",
    )
    .await
}

pub async fn ensure_ffmpeg(ffmpeg: &Path) -> Result<PathBuf> {
    if let Some(found) = probe_ffmpeg(ffmpeg).await? {
        return Ok(found);
    }

    println!("本地未找到 ffmpeg，正在自动下载...");
    let cache_dir = app_cache_dir().join("ffmpeg");
    fs::create_dir_all(&cache_dir)
        .await
        .context("创建 ffmpeg 缓存目录失败")?;

    let exe_path = cache_dir.join("ffmpeg.exe");
    if let Some(found) = probe_ffmpeg(&exe_path).await? {
        return Ok(found);
    }

    let zip_path = cache_dir.join("ffmpeg.zip");
    download_file(FFMPEG_BUILD_URL, &zip_path, "ffmpeg").await?;
    println!("正在解压 ffmpeg...");
    extract_ffmpeg_from_zip(&zip_path, &cache_dir)?;
    let _ = fs::remove_file(&zip_path).await;

    let found = probe_ffmpeg(&exe_path)
        .await?
        .context("ffmpeg 已下载但未找到 ffmpeg.exe")?;
    println!("ffmpeg 已就绪：{}", found.display());
    Ok(found)
}

pub async fn write_concat_list(list_path: &Path, paths: &[PathBuf]) -> Result<()> {
    let mut text = String::new();
    for path in paths {
        let escaped = path
            .to_string_lossy()
            .replace('\\', "/")
            .replace('\'', "'\\''");
        text.push_str("file '");
        text.push_str(&escaped);
        text.push_str("'\n");
    }

    fs::write(list_path, text)
        .await
        .with_context(|| format!("写入文件失败：{}", list_path.display()))
}

async fn run_ffmpeg<I, S>(ffmpeg: &Path, args: I, context: &str) -> Result<()>
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    let output = Command::new(ffmpeg)
        .args(args)
        .output()
        .await
        .with_context(|| format!("运行 {} 失败", ffmpeg.display()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("{context}: {}", stderr.trim());
    }

    Ok(())
}

async fn probe_ffmpeg(ffmpeg: &Path) -> Result<Option<PathBuf>> {
    let candidate = if ffmpeg.is_dir() {
        ffmpeg.join("ffmpeg.exe")
    } else {
        ffmpeg.to_path_buf()
    };

    let status = Command::new(&candidate)
        .arg("-version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await;
    match status {
        Ok(status) if status.success() => Ok(Some(candidate)),
        Ok(_) | Err(_) => Ok(None),
    }
}

async fn download_file(url: &str, dest: &Path, label: &str) -> Result<()> {
    let client: Client = build_client(None)?;
    let response = client
        .get(url)
        .header(REFERER, ROOT_REFERER)
        .send()
        .await
        .context("下载 ffmpeg 失败")?
        .error_for_status()
        .context("ffmpeg 下载请求失败")?;

    let total = response.content_length().unwrap_or_default();
    let progress = if total > 0 {
        let pb = ProgressBar::new(total);
        pb.set_style(
            ProgressStyle::with_template(
                "{msg:>10} [{bar:40.cyan/blue}] {bytes}/{total_bytes} {bytes_per_sec}",
            )
            .expect("valid progress template"),
        );
        pb.set_message(label.to_owned());
        Some(pb)
    } else {
        println!("正在下载 {label}...");
        None
    };

    let mut file = File::create(dest)
        .await
        .with_context(|| format!("创建文件失败：{}", dest.display()))?;
    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.context("下载 ffmpeg 时失败")?;
        file.write_all(&chunk)
            .await
            .with_context(|| format!("写入文件失败：{}", dest.display()))?;
        if let Some(pb) = &progress {
            pb.inc(chunk.len() as u64);
        }
    }

    file.flush()
        .await
        .with_context(|| format!("刷新文件失败：{}", dest.display()))?;
    if let Some(pb) = progress {
        pb.finish_with_message(format!("{label} 下载完成"));
    }
    Ok(())
}

fn extract_ffmpeg_from_zip(zip_path: &Path, out_dir: &Path) -> Result<()> {
    let file = StdFile::open(zip_path)
        .with_context(|| format!("打开文件失败：{}", zip_path.display()))?;
    let mut archive = ZipArchive::new(file).context("ffmpeg 压缩包无效")?;

    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).context("压缩包条目损坏")?;
        let name = entry.name().replace('\\', "/");
        if !name.ends_with("bin/ffmpeg.exe") {
            continue;
        }

        let out_path = out_dir.join("ffmpeg.exe");
        let mut out = StdFile::create(&out_path)
            .with_context(|| format!("创建文件失败：{}", out_path.display()))?;
        std::io::copy(&mut entry, &mut out).context("解压 ffmpeg.exe 失败")?;
        return Ok(());
    }

    bail!("下载的压缩包里没有找到 ffmpeg.exe");
}

fn app_cache_dir() -> PathBuf {
    if let Ok(local_app_data) = env::var("LOCALAPPDATA") {
        return PathBuf::from(local_app_data).join("bilibili-downloader");
    }
    PathBuf::from(".bilibili-downloader")
}
