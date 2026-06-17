use std::collections::HashSet;

use anyhow::{bail, Context, Result};
use regex::Regex;
use reqwest::{
    header::{
        HeaderMap, HeaderValue, ACCEPT, ACCEPT_LANGUAGE, COOKIE, REFERER, USER_AGENT,
    },
    Client,
};

use crate::{
    cli::Args,
    constants::{ROOT_REFERER, USER_AGENT_VALUE},
    models::{ApiResponse, PlayData, VideoId, ViewData},
};

pub struct CollectionEntries {
    pub title: String,
    pub items: Vec<CollectionEntry>,
}

pub struct CollectionEntry {
    pub bvid: String,
}

pub async fn fetch_view(client: &Client, video_id: &VideoId) -> Result<ViewData> {
    let url = match video_id {
        VideoId::Bvid(bvid) => {
            format!("https://api.bilibili.com/x/web-interface/view?bvid={bvid}")
        }
        VideoId::Aid(aid) => {
            format!("https://api.bilibili.com/x/web-interface/view?aid={aid}")
        }
    };

    let response = client
        .get(url)
        .header(REFERER, ROOT_REFERER)
        .send()
        .await
        .context("请求视频信息失败")?
        .error_for_status()
        .context("视频信息请求失败")?
        .json::<ApiResponse<ViewData>>()
        .await
        .context("解析视频信息失败")?;

    response.into_data()
}

pub async fn fetch_play_url(client: &Client, bvid: &str, cid: u64, qn: u32) -> Result<PlayData> {
    let url = format!(
        "https://api.bilibili.com/x/player/playurl?bvid={bvid}&cid={cid}&qn={qn}&fnver=0&fnval=4048&fourk=1"
    );

    let response = client
        .get(url)
        .header(REFERER, format!("https://www.bilibili.com/video/{bvid}/"))
        .send()
        .await
        .context("请求播放地址失败")?
        .error_for_status()
        .context("播放地址请求失败")?
        .json::<ApiResponse<PlayData>>()
        .await
        .context("解析播放地址失败")?;

    response.into_data()
}

pub fn build_client(cookie: Option<&str>) -> Result<Client> {
    let mut headers = HeaderMap::new();
    headers.insert(USER_AGENT, HeaderValue::from_static(USER_AGENT_VALUE));
    headers.insert(ACCEPT, HeaderValue::from_static("*/*"));
    headers.insert(
        ACCEPT_LANGUAGE,
        HeaderValue::from_static("zh-CN,zh;q=0.9,en;q=0.8"),
    );

    if let Some(cookie) = cookie {
        headers.insert(
            COOKIE,
            HeaderValue::from_str(cookie).context("Cookie contains invalid characters")?,
        );
    }

    Client::builder()
        .default_headers(headers)
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()
        .context("构建 HTTP 客户端失败")
}

pub fn load_cookie(args: &Args) -> Result<Option<String>> {
    if let Some(cookie) = &args.cookie {
        return Ok(Some(clean_cookie_header(cookie)));
    }

    if let Some(path) = &args.cookie_file {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("读取 Cookie 文件失败：{}", path.display()))?;
        let cookie = parse_cookie_file(&text);
        if cookie.is_empty() {
            bail!("Cookie 文件里没有可用 Cookie。");
        }
        return Ok(Some(cookie));
    }

    Ok(None)
}

pub fn parse_video_id(input: &str) -> Result<VideoId> {
    let bv_re = Regex::new(r"(?i)\bBV[0-9A-Za-z]{10}\b").expect("valid BV regex");
    if let Some(found) = bv_re.find(input) {
        return Ok(VideoId::Bvid(found.as_str().to_owned()));
    }

    let av_re = Regex::new(r"(?i)(?:^|[^\w])av(\d+)\b").expect("valid av regex");
    if let Some(captures) = av_re.captures(input) {
        let aid = captures[1].parse().context("av 号无效")?;
        return Ok(VideoId::Aid(aid));
    }

    if input.chars().all(|ch| ch.is_ascii_digit()) {
        let aid = input.parse().context("数字 aid 无效")?;
        return Ok(VideoId::Aid(aid));
    }

    bail!("输入里没有找到 BV 号或 av 号。");
}

pub fn extract_page_from_input(input: &str) -> Option<usize> {
    let page_re = Regex::new(r"(?:[?&])p=(\d+)").expect("valid page regex");
    page_re
        .captures(input)
        .and_then(|captures| captures.get(1))
        .and_then(|page| page.as_str().parse::<usize>().ok())
        .filter(|page| *page > 0)
}

pub fn collect_collection_entries(view: &ViewData) -> Option<CollectionEntries> {
    if view.pages.len() > 1 {
        return None;
    }

    let season = view.ugc_season.as_ref()?;
    let mut seen = HashSet::new();
    let mut items = Vec::new();

    for section in &season.sections {
        for episode in &section.episodes {
            if seen.insert(episode.aid) {
                items.push(CollectionEntry {
                    bvid: episode.bvid.clone(),
                });
            }
        }
    }

    if items.len() <= 1 {
        return None;
    }

    Some(CollectionEntries {
        title: if season.title.trim().is_empty() {
            view.title.clone()
        } else {
            season.title.clone()
        },
        items,
    })
}

fn parse_cookie_file(text: &str) -> String {
    let lines: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect();

    let netscape_pairs: Vec<String> = lines
        .iter()
        .filter_map(|line| {
            let fields: Vec<&str> = line.split('\t').collect();
            if fields.len() >= 7 {
                Some(format!("{}={}", fields[5].trim(), fields[6].trim()))
            } else {
                None
            }
        })
        .collect();

    if !netscape_pairs.is_empty() {
        return netscape_pairs.join("; ");
    }

    clean_cookie_header(&lines.join("; "))
}

fn clean_cookie_header(cookie: &str) -> String {
    cookie
        .trim()
        .trim_start_matches("Cookie:")
        .trim()
        .trim_end_matches(';')
        .to_owned()
}
