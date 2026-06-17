use std::{cmp::Reverse, path::Path};

use anyhow::{bail, Context, Result};
use futures_util::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::{header::REFERER, Client};
use tokio::{
    fs::{self, File},
    io::AsyncWriteExt,
};

use crate::{
    api::fetch_play_url,
    cli::{Args, OutputFormat},
    ffmpeg::{
        mux_mp4, remux_audio_m4a, remux_concat, remux_single, transcode_concat_mp3,
        transcode_mp3, write_concat_list,
    },
    models::{DashData, DashStream, DurlData, PageData, ViewData},
    util::{make_output_stem, quality_label, truncate_chars, unique_path, unix_timestamp},
};

pub async fn download_video(
    client: &Client,
    args: &Args,
    ffmpeg: &Path,
    format: OutputFormat,
    target_qn: u32,
    view: &ViewData,
    selected_page: Option<usize>,
) -> Result<()> {
    let referer = format!("https://www.bilibili.com/video/{}/", view.bvid);

    if view.pages.len() > 1 {
        println!("检测到多 P 视频，共 {} P，开始全部下载。", view.pages.len());
        for (index, page) in view.pages.iter().enumerate() {
            println!();
            println!("正在下载第 {}/{} P：{}", index + 1, view.pages.len(), page.part);
            download_page(client, args, ffmpeg, format, target_qn, view, page, &referer).await?;
        }
        return Ok(());
    }

    let page_index = selected_page.unwrap_or(1);
    let page = view
        .pages
        .get(page_index.saturating_sub(1))
        .with_context(|| format!("第 {} P 不存在。这个视频共 {} P。", page_index, view.pages.len()))?;
    download_page(client, args, ffmpeg, format, target_qn, view, page, &referer).await
}

pub async fn download_page(
    client: &Client,
    args: &Args,
    ffmpeg: &Path,
    format: OutputFormat,
    target_qn: u32,
    view: &ViewData,
    page: &PageData,
    referer: &str,
) -> Result<()> {
    let play = fetch_play_url(client, &view.bvid, page.cid, target_qn).await?;
    let stem = make_output_stem(view, page);
    let output_path = unique_path(
        args.output_dir
            .join(format!("{}.{}", stem, format.extension())),
    )
    .await;
    let temp_dir = args.output_dir.join(".bili_tmp").join(format!(
        "{}_{}",
        truncate_chars(&stem, 40),
        unix_timestamp()
    ));
    fs::create_dir_all(&temp_dir)
        .await
        .with_context(|| format!("创建目录失败：{}", temp_dir.display()))?;

    println!("分页：{} - {}", page.page, page.part);
    println!("输出：{}", output_path.display());

    let result = match play.dash {
        Some(dash) => {
            handle_dash(
                client,
                ffmpeg,
                format,
                target_qn,
                &dash,
                referer,
                &temp_dir,
                &output_path,
            )
            .await
        }
        None => {
            handle_durl(
                client,
                ffmpeg,
                format,
                play.durl.as_deref().unwrap_or_default(),
                referer,
                &temp_dir,
                &output_path,
            )
            .await
        }
    };

    if !args.keep_temp {
        let _ = fs::remove_dir_all(&temp_dir).await;
    } else {
        println!("已保留临时文件：{}", temp_dir.display());
    }

    result?;
    println!("完成：{}", output_path.display());
    Ok(())
}

async fn handle_dash(
    client: &Client,
    ffmpeg: &Path,
    format: OutputFormat,
    target_qn: u32,
    dash: &DashData,
    referer: &str,
    temp_dir: &Path,
    output_path: &Path,
) -> Result<()> {
    match format {
        OutputFormat::Mp4 => {
            let video = select_video_stream(&dash.video, target_qn)?;
            let audio = select_audio_stream(dash.audio.as_deref())?;
            print_stream_choice("Video", video, target_qn);
            print_stream_choice("Audio", audio, audio.id);

            let video_path = temp_dir.join("video.m4s");
            let audio_path = temp_dir.join("audio.m4s");
            let video_urls = video.urls();
            let audio_urls = audio.urls();
            tokio::try_join!(
                download_media(client, &video_urls, referer, &video_path, "video"),
                download_media(client, &audio_urls, referer, &audio_path, "audio"),
            )?;
            mux_mp4(ffmpeg, &video_path, &audio_path, output_path).await
        }
        OutputFormat::Mp3 => {
            let audio = select_audio_stream(dash.audio.as_deref())?;
            print_stream_choice("Audio", audio, audio.id);

            let audio_path = temp_dir.join("audio.m4s");
            let remuxed_audio_path = temp_dir.join("audio.m4a");
            println!("中间音频：{}", remuxed_audio_path.display());
            download_media(client, &audio.urls(), referer, &audio_path, "audio").await?;
            remux_audio_m4a(ffmpeg, &audio_path, &remuxed_audio_path).await?;
            transcode_mp3(ffmpeg, &remuxed_audio_path, output_path).await
        }
    }
}

async fn handle_durl(
    client: &Client,
    ffmpeg: &Path,
    format: OutputFormat,
    durl: &[DurlData],
    referer: &str,
    temp_dir: &Path,
    output_path: &Path,
) -> Result<()> {
    if durl.is_empty() {
        bail!("没有返回 DASH 或 durl 视频流。");
    }

    println!("使用旧版 durl 视频流。");
    let mut segment_paths = Vec::with_capacity(durl.len());
    for (index, segment) in durl.iter().enumerate() {
        let segment_path = temp_dir.join(format!("segment_{:03}.bin", index + 1));
        download_media(
            client,
            &segment.urls(),
            referer,
            &segment_path,
            &format!("segment {}", index + 1),
        )
        .await?;
        segment_paths.push(segment_path);
    }

    if segment_paths.len() == 1 {
        return match format {
            OutputFormat::Mp4 => remux_single(ffmpeg, &segment_paths[0], output_path).await,
            OutputFormat::Mp3 => {
                let remuxed_audio_path = temp_dir.join("audio.m4a");
                println!("中间音频：{}", remuxed_audio_path.display());
                remux_audio_m4a(ffmpeg, &segment_paths[0], &remuxed_audio_path).await?;
                transcode_mp3(ffmpeg, &remuxed_audio_path, output_path).await
            }
        };
    }

    let list_path = temp_dir.join("concat.txt");
    write_concat_list(&list_path, &segment_paths).await?;
    match format {
        OutputFormat::Mp4 => remux_concat(ffmpeg, &list_path, output_path).await,
        OutputFormat::Mp3 => transcode_concat_mp3(ffmpeg, &list_path, output_path).await,
    }
}

fn select_video_stream(streams: &[DashStream], target_qn: u32) -> Result<&DashStream> {
    streams
        .iter()
        .filter(|stream| stream.id <= target_qn)
        .max_by_key(|stream| {
            (
                stream.id,
                codec_preference(stream),
                stream.bandwidth.unwrap_or_default(),
            )
        })
        .or_else(|| {
            streams.iter().min_by_key(|stream| {
                (
                    stream.id,
                    Reverse(codec_preference(stream)),
                    Reverse(stream.bandwidth.unwrap_or_default()),
                )
            })
        })
        .context("No video stream was returned.")
}

fn select_audio_stream(streams: Option<&[DashStream]>) -> Result<&DashStream> {
    streams
        .unwrap_or_default()
        .iter()
        .max_by_key(|stream| stream.bandwidth.unwrap_or_default())
        .context("No audio stream was returned.")
}

fn codec_preference(stream: &DashStream) -> u8 {
    let Some(codecs) = &stream.codecs else {
        return 0;
    };

    let codecs = codecs.to_ascii_lowercase();
    if codecs.contains("avc") {
        3
    } else if codecs.contains("hev") || codecs.contains("hvc") {
        2
    } else if codecs.contains("av01") {
        1
    } else {
        0
    }
}

fn print_stream_choice(label: &str, stream: &DashStream, requested_qn: u32) {
    let actual = quality_label(stream.id);
    if stream.id != requested_qn && label == "Video" {
        println!(
            "{label}：请求 {}，实际使用 {}",
            quality_label(requested_qn),
            actual
        );
    } else {
        println!("{label}：{actual}");
    }

    if let (Some(width), Some(height)) = (stream.width, stream.height) {
        println!("分辨率：{}x{}", width, height);
    }
    if let Some(codecs) = &stream.codecs {
        println!("编码：{codecs}");
    }
    if let Some(mime_type) = stream.mime_type() {
        println!("类型：{mime_type}");
    }
}

async fn download_media(
    client: &Client,
    urls: &[String],
    referer: &str,
    path: &Path,
    label: &str,
) -> Result<()> {
    let mut last_error: Option<anyhow::Error> = None;

    for url in urls {
        match download_one(client, url, referer, path, label).await {
            Ok(()) => return Ok(()),
            Err(error) => {
                last_error = Some(error);
                let _ = fs::remove_file(path).await;
            }
        }
    }

    Err(last_error.unwrap_or_else(|| anyhow::anyhow!("没有可用下载地址")))
}

async fn download_one(
    client: &Client,
    url: &str,
    referer: &str,
    path: &Path,
    label: &str,
) -> Result<()> {
    let response = client
        .get(url)
        .header(REFERER, referer)
        .send()
        .await
        .with_context(|| format!("开始下载 {label} 失败"))?
        .error_for_status()
        .with_context(|| format!("{label} 下载请求失败"))?;

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

    let mut file = File::create(path)
        .await
        .with_context(|| format!("创建文件失败：{}", path.display()))?;
    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.with_context(|| format!("下载 {label} 时失败"))?;
        file.write_all(&chunk)
            .await
            .with_context(|| format!("写入文件失败：{}", path.display()))?;
        if let Some(pb) = &progress {
            pb.inc(chunk.len() as u64);
        }
    }

    file.flush()
        .await
        .with_context(|| format!("刷新文件失败：{}", path.display()))?;
    if let Some(pb) = progress {
        pb.finish_and_clear();
    }
    Ok(())
}
