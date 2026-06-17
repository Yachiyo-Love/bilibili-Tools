use anyhow::{bail, Context, Result};
use serde::Deserialize;

#[derive(Debug, Clone)]
pub enum VideoId {
    Bvid(String),
    Aid(u64),
}

#[derive(Debug, Deserialize)]
pub struct ApiResponse<T> {
    pub code: i64,
    #[serde(default, alias = "msg")]
    pub message: String,
    pub data: Option<T>,
}

impl<T> ApiResponse<T> {
    pub fn into_data(self) -> Result<T> {
        if self.code != 0 {
            bail!("Bilibili API error {}: {}", self.code, self.message);
        }

        self.data.context("B 站接口没有返回数据")
    }
}

#[derive(Debug, Deserialize)]
pub struct ViewData {
    pub aid: u64,
    pub bvid: String,
    pub title: String,
    pub pages: Vec<PageData>,
    #[serde(default)]
    pub ugc_season: Option<UgcSeasonData>,
}

#[derive(Debug, Deserialize)]
pub struct PageData {
    pub cid: u64,
    pub page: u32,
    pub part: String,
}

#[derive(Debug, Deserialize)]
pub struct UgcSeasonData {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub sections: Vec<UgcSeasonSectionData>,
}

#[derive(Debug, Deserialize)]
pub struct UgcSeasonSectionData {
    #[serde(default)]
    pub episodes: Vec<UgcSeasonEpisodeData>,
}

#[derive(Debug, Deserialize)]
pub struct UgcSeasonEpisodeData {
    pub aid: u64,
    pub bvid: String,
}

#[derive(Debug, Deserialize)]
pub struct PlayData {
    #[serde(default)]
    pub accept_quality: Vec<u32>,
    #[serde(default)]
    pub accept_description: Vec<String>,
    pub dash: Option<DashData>,
    #[serde(default)]
    pub durl: Option<Vec<DurlData>>,
}

#[derive(Debug, Deserialize)]
pub struct DashData {
    #[serde(default)]
    pub video: Vec<DashStream>,
    #[serde(default)]
    pub audio: Option<Vec<DashStream>>,
}

#[derive(Debug, Deserialize)]
pub struct DashStream {
    pub id: u32,
    #[serde(default, rename = "baseUrl")]
    pub base_url_camel: Option<String>,
    #[serde(default, rename = "base_url")]
    pub base_url_snake: Option<String>,
    #[serde(default, rename = "backupUrl")]
    pub backup_url_camel: Option<Vec<String>>,
    #[serde(default, rename = "backup_url")]
    pub backup_url_snake: Option<Vec<String>>,
    #[serde(default)]
    pub bandwidth: Option<u64>,
    #[serde(default)]
    pub codecs: Option<String>,
    #[serde(default, rename = "mimeType")]
    pub mime_type_camel: Option<String>,
    #[serde(default, rename = "mime_type")]
    pub mime_type_snake: Option<String>,
    #[serde(default)]
    pub width: Option<u32>,
    #[serde(default)]
    pub height: Option<u32>,
}

impl DashStream {
    pub fn urls(&self) -> Vec<String> {
        let mut urls = Vec::new();
        if let Some(url) = self
            .base_url_camel
            .as_ref()
            .or(self.base_url_snake.as_ref())
        {
            urls.push(url.clone());
        }
        if let Some(backup_urls) = self
            .backup_url_camel
            .as_ref()
            .or(self.backup_url_snake.as_ref())
        {
            urls.extend(backup_urls.iter().cloned());
        }
        urls
    }

    pub fn mime_type(&self) -> Option<&str> {
        self.mime_type_camel
            .as_deref()
            .or(self.mime_type_snake.as_deref())
    }
}

#[derive(Debug, Deserialize)]
pub struct DurlData {
    pub url: String,
    #[serde(default, rename = "backupUrl")]
    pub backup_url_camel: Option<Vec<String>>,
    #[serde(default, rename = "backup_url")]
    pub backup_url_snake: Option<Vec<String>>,
}

impl DurlData {
    pub fn urls(&self) -> Vec<String> {
        let mut urls = Vec::new();
        urls.push(self.url.clone());
        if let Some(backup_urls) = self
            .backup_url_camel
            .as_ref()
            .or(self.backup_url_snake.as_ref())
        {
            urls.extend(backup_urls.iter().cloned());
        }
        urls
    }
}
