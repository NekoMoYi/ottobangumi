use regex::Regex;
use reqwest;
use scraper::{Html, Selector};
use std::error::Error;
use thiserror::Error;

type BoxErr = Box<dyn Error + Send + Sync>;
const MIKAN_URL: &str = "https://mikanani.me";

#[derive(Error, Debug)]
enum MikanError {
    #[error("Parse error")]
    ParseError,
    #[error("Title not found")]
    TitleNotFound,
    #[error("Id not found")]
    IdNotFound,
    #[error("Poster not found")]
    PosterNotFound,
    #[error("Week day not found")]
    WeekDayNotFound,
    #[error("Magnet not found")]
    MagnetNotFound,
}

#[derive(Debug, Default)]
pub struct BangumiInfo {
    pub id: u32,
    pub title: String,
    pub weekday: u8,
    pub poster_url: String,
    pub magnet: Option<String>,
}

pub struct MikanParser {
    client: reqwest::Client,
}

impl MikanParser {
    pub fn new() -> Self {
        let client = reqwest::Client::new();
        Self { client }
    }
    pub fn set_proxy(&mut self, proxy: Option<reqwest::Proxy>) -> Result<&Self, reqwest::Error> {
        self.client = match proxy {
            Some(p) => reqwest::Client::builder().proxy(p).build()?,
            None => reqwest::Client::new(),
        };
        Ok(self)
    }
    pub async fn from_id(&self, id: u32) -> Result<BangumiInfo, BoxErr> {
        let resp = self
            .client
            .get(format!("{}/Home/Bangumi/{}", MIKAN_URL, id))
            .send()
            .await?;
        let text = resp.text().await?;
        let document = Html::parse_document(&text);
        let bangumi = Self::parse_document(&document)?;
        if bangumi.id != id {
            return Err(Box::new(MikanError::ParseError));
        }
        Ok(bangumi)
    }
    pub async fn from_rss_url(&self, url: &str) -> Result<BangumiInfo, BoxErr> {
        let delimiters = ['?', '&'];
        let id = url
            .split_terminator(|c| delimiters.contains(&c))
            .find(|x| x.starts_with("bangumiId="))
            .and_then(|x| x.split('=').nth(1))
            .ok_or(MikanError::IdNotFound)?
            .parse::<u32>()?;
        self.from_id(id).await
    }
    pub async fn from_url(&self, url: &str) -> Result<BangumiInfo, BoxErr> {
        let resp = self.client.get(url).send().await?;
        let text = resp.text().await?;
        let document = Html::parse_document(&text);
        let id = Self::parse_id(&document)?;
        let magnet = if url.contains("/Home/Episode/") {
            Some(Self::parse_magnet(&document)?)
        } else {
            None
        };
        let mut bangumi = self.from_id(id).await?;
        bangumi.magnet = magnet;
        Ok(bangumi)
    }
    pub fn parse_document(document: &Html) -> Result<BangumiInfo, BoxErr> {
        let bangumi_info = BangumiInfo {
            id: Self::parse_id(document)?,
            title: Self::parse_title(document)?,
            poster_url: Self::parse_poster_url(document)?,
            weekday: Self::parse_week_day(document)?,
            magnet: None,
        };
        Ok(bangumi_info)
    }
    fn parse_title(document: &Html) -> Result<String, BoxErr> {
        let title_selector = Selector::parse("p.bangumi-title").unwrap();
        let title = document
            .select(&title_selector)
            .next()
            .ok_or(MikanError::TitleNotFound)?
            .text()
            .collect::<String>()
            .trim()
            .to_string();
        Ok(title)
    }
    fn parse_poster_url(document: &Html) -> Result<String, BoxErr> {
        let poster_selector = Selector::parse("div.bangumi-poster").unwrap();
        let poster_url = document
            .select(&poster_selector)
            .next()
            .and_then(|x| x.value().attr("style"))
            .and_then(|x| {
                x.split('\'')
                    .nth(1)
                    .and_then(|url| url.split('?').nth(0))
                    .map(|url| format!("{}{}", MIKAN_URL, url))
            })
            .ok_or(MikanError::PosterNotFound)?;
        Ok(poster_url)
    }
    fn parse_week_day(document: &Html) -> Result<u8, BoxErr> {
        let info_selector = Selector::parse("p.bangumi-info").unwrap();
        let info_text = document
            .select(&info_selector)
            .find(|x| x.text().any(|x| x.starts_with("放送日期")))
            .ok_or(MikanError::WeekDayNotFound)?
            .text()
            .collect::<String>();
        let week_days = vec!['一', '二', '三', '四', '五', '六', '日'];
        let ch = info_text
            .chars()
            .last()
            .ok_or(MikanError::WeekDayNotFound)?;
        let weekday = week_days
            .iter()
            .position(|&x| x == ch)
            .ok_or(MikanError::WeekDayNotFound)? as u8
            + 1;
        Ok(weekday)
    }
    fn parse_id(document: &Html) -> Result<u32, BoxErr> {
        let id_selector = Selector::parse("a.mikan-rss").unwrap();
        let id_pattern = Regex::new(r"bangumiId=(\d+)").unwrap();
        let href = document
            .select(&id_selector)
            .next()
            .and_then(|x| x.value().attr("href"))
            .ok_or(MikanError::IdNotFound)?;
        if let Some(id) = id_pattern.captures(href) {
            if let Some(id) = id.get(1) {
                return Ok(id.as_str().parse::<u32>()?);
            }
        }
        Err(Box::new(MikanError::IdNotFound))
    }
    fn parse_magnet(document: &Html) -> Result<String, BoxErr> {
        let magnet_selector = Selector::parse("a.episode-btn").unwrap();
        let magnet = document
            .select(&magnet_selector)
            .filter_map(|x| x.value().attr("href"))
            .find(|x| x.starts_with("magnet:"))
            .ok_or(MikanError::MagnetNotFound)?;
        Ok(magnet.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dotenv::dotenv;
    use std::env;

    #[tokio::test]
    async fn test_bangumi() -> Result<(), BoxErr> {
        let mut parser = MikanParser::new();
        dotenv().ok();
        let mut proxy: Option<reqwest::Proxy> = None;
        if let Some(proxy_url) = env::var("PROXY_URL").ok() {
            proxy = Some(reqwest::Proxy::all(proxy_url)?);
        }
        parser.set_proxy(proxy.clone())?;
        let bangumi = parser.from_id(2353).await?;
        assert_eq!(bangumi.id, 2353);
        assert_eq!(bangumi.title, "无职转生～到了异世界就拿出真本事～");
        assert_eq!(bangumi.weekday, 7);
        assert_eq!(
            bangumi.poster_url,
            format!("{}{}", MIKAN_URL, "/images/Bangumi/202101/2acbca31.jpg")
        );

        let bangumi = parser
            .from_url("https://mikanani.me/Home/Episode/72d528cc2048bbdf0468a4265ee1abde62793fa0")
            .await?;
        assert_eq!(bangumi.id, 3330);
        assert_eq!(bangumi.title, "狼与香辛料 行商邂逅贤狼");
        assert_eq!(bangumi.weekday, 1);
        Ok(())
    }
}
