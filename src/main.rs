mod api;
mod cli;
mod constants;
mod download;
mod ffmpeg;
mod models;
mod util;

use anyhow::{Context, Result};
use clap::Parser;
use tokio::fs;

use crate::{
    api::{
        build_client, collect_collection_entries, extract_page_from_input, fetch_play_url,
        fetch_view, load_cookie, parse_video_id,
    },
    cli::{prompt_quality_choice, resolve_format, resolve_input, Args, OutputFormat},
    download::download_video,
    ffmpeg::ensure_ffmpeg,
    models::VideoId,
    util::format_accept_quality,
};

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let (input, prompted_input) = resolve_input(args.input.clone())?;
    let input_page = extract_page_from_input(&input);
    let (format, _) = resolve_format(args.format)?;

    let ffmpeg = ensure_ffmpeg(&args.ffmpeg).await?;
    let cookie = load_cookie(&args)?;
    let client = build_client(cookie.as_deref())?;
    let video_id = parse_video_id(&input)?;
    let view = fetch_view(&client, &video_id).await?;

    let mut target_qn = args.quality.qn();
    let first_page = view.pages.first().context("视频没有分页数据")?;
    let first_play = fetch_play_url(&client, &view.bvid, first_page.cid, target_qn).await?;

    if format == OutputFormat::Mp4 && !first_play.accept_quality.is_empty() {
        println!(
            "可用清晰度：{}",
            format_accept_quality(&first_play.accept_quality, &first_play.accept_description)
        );

        if prompted_input {
            if let Some(selected_qn) = prompt_quality_choice(
                &first_play.accept_quality,
                &first_play.accept_description,
                target_qn,
            )? {
                target_qn = selected_qn;
            }
        }
    }

    fs::create_dir_all(&args.output_dir)
        .await
        .with_context(|| format!("创建目录失败：{}", args.output_dir.display()))?;

    println!("标题：{}", view.title);
    if let Some(collection_entries) = collect_collection_entries(&view) {
        println!(
            "检测到合集：{}，共 {} 个视频，开始全部下载。",
            collection_entries.title,
            collection_entries.items.len()
        );
        for (index, item) in collection_entries.items.iter().enumerate() {
            println!();
            println!(
                "正在下载合集第 {}/{} 个视频。",
                index + 1,
                collection_entries.items.len()
            );
            let item_view = fetch_view(&client, &VideoId::Bvid(item.bvid.clone())).await?;
            download_video(&client, &args, &ffmpeg, format, target_qn, &item_view, None).await?;
        }
    } else {
        download_video(
            &client,
            &args,
            &ffmpeg,
            format,
            target_qn,
            &view,
            args.page.or(input_page),
        )
        .await?;
    }

    println!("全部完成。");
    Ok(())
}
