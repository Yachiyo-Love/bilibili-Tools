use std::{
    io::{self, Write},
    path::PathBuf,
};

use anyhow::{bail, Context, Result};
use clap::{Parser, ValueEnum};

use crate::util::quality_label;

#[derive(Parser, Debug)]
#[command(version, about = "下载 B 站视频为 MP4 或 MP3。")]
pub struct Args {
    pub input: Option<String>,
    #[arg(short, long, value_enum)]
    pub format: Option<OutputFormat>,
    #[arg(short, long, value_enum, default_value_t = Quality::P1080)]
    pub quality: Quality,
    #[arg(short, long)]
    pub page: Option<usize>,
    #[arg(short, long, default_value = "downloads")]
    pub output_dir: PathBuf,
    #[arg(long, env = "BILI_COOKIE")]
    pub cookie: Option<String>,
    #[arg(long)]
    pub cookie_file: Option<PathBuf>,
    #[arg(long, default_value = "ffmpeg")]
    pub ffmpeg: PathBuf,
    #[arg(long)]
    pub keep_temp: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    Mp4,
    Mp3,
}

impl OutputFormat {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Mp4 => "mp4",
            Self::Mp3 => "mp3",
        }
    }
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum Quality {
    #[value(name = "4k", alias = "4K")]
    Q4k,
    #[value(name = "1080p", alias = "1080P")]
    P1080,
    #[value(name = "720p", alias = "720P")]
    P720,
    #[value(name = "360p", alias = "360P")]
    P360,
}

impl Quality {
    pub fn qn(self) -> u32 {
        match self {
            Self::Q4k => 120,
            Self::P1080 => 80,
            Self::P720 => 64,
            Self::P360 => 16,
        }
    }
}

pub fn resolve_input(input: Option<String>) -> Result<(String, bool)> {
    if let Some(input) = input {
        return Ok((input, false));
    }

    print!("请输入 BV/AV 号或视频链接：");
    io::stdout().flush().ok();

    let mut buffer = String::new();
    io::stdin()
        .read_line(&mut buffer)
        .context("读取输入失败")?;

    let input = buffer.trim().to_owned();
    if input.is_empty() {
        bail!("未输入内容。");
    }

    Ok((input, true))
}

pub fn prompt_quality_choice(
    qualities: &[u32],
    descriptions: &[String],
    default_qn: u32,
) -> Result<Option<u32>> {
    println!("请选择清晰度：");
    for (index, quality) in qualities.iter().enumerate() {
        let label = descriptions
            .get(index)
            .cloned()
            .unwrap_or_else(|| quality_label(*quality));
        println!("  {}. {}", index + 1, label);
    }

    print!(
        "清晰度编号（直接回车使用 {}）：",
        quality_label(default_qn)
    );
    io::stdout().flush().ok();

    let mut buffer = String::new();
    io::stdin()
        .read_line(&mut buffer)
        .context("读取清晰度选择失败")?;

    let choice = buffer.trim();
    if choice.is_empty() {
        return Ok(None);
    }

    let index: usize = choice
        .parse()
        .with_context(|| format!("清晰度选择无效：{choice}"))?;
    if index == 0 || index > qualities.len() {
        bail!("清晰度编号必须在 1 到 {} 之间。", qualities.len());
    }

    Ok(Some(qualities[index - 1]))
}

pub fn resolve_format(format: Option<OutputFormat>) -> Result<(OutputFormat, bool)> {
    if let Some(format) = format {
        return Ok((format, false));
    }

    println!("请选择输出格式：");
    println!("  1. MP4");
    println!("  2. MP3");
    print!("格式编号（直接回车使用 MP4）：");
    io::stdout().flush().ok();

    let mut buffer = String::new();
    io::stdin()
        .read_line(&mut buffer)
        .context("读取格式选择失败")?;

    let choice = buffer.trim();
    if choice.is_empty() {
        return Ok((OutputFormat::Mp4, true));
    }

    let format = match choice {
        "1" => OutputFormat::Mp4,
        "2" => OutputFormat::Mp3,
        _ => bail!("格式编号只能是 1 或 2。"),
    };

    Ok((format, true))
}
