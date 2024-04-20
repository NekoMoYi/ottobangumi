// Edited from EstrellaXD/Auto_Bangumi.git

use regex::Regex;
use thiserror::Error;

const TITLE_PATTERN: &str = r"(.*|\[.*])( -? \d+|\[\d+]|\[\d+.?[vV]\d]|第\d+[话話集]|\[第?\d+[话話集]]|\[\d+.?END]|[Ee][Pp]?\d+)(.*)";
const EPISODE_PATTERN: &str = r"\d+";
const PREFIX_PATTERN: &str = r"[^\w\s\u4e00-\u9fff\u3040-\u309f\u30a0-\u30ff-]";
const FANSUB_PATTERN: &str = r"[\[\]]";

type BoxErr = Box<dyn std::error::Error + Send + Sync>;

#[derive(Default, Debug, PartialEq)]
pub struct ParseResult {
    pub title_zh: String,
    pub title_en: String,
    pub title_jp: String,
    pub season: i8,
    pub episode: i16,
    pub fansub: String,
}

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("Invalid input")]
    InvalidInput,
    #[error("Failed to parse season info")]
    InvalidSeason,
    #[error("Failed to parse title")]
    InvalidTitle,
    #[error("Failed to parse episode")]
    InvalidEpisode,
}

fn build_regex(re: &str) -> Regex {
    Regex::new(re).unwrap()
}

fn parse_fansub(title: &str) -> Option<String> {
    build_regex(FANSUB_PATTERN)
        .split(title)
        .nth(1)
        .map(|s| s.to_string())
}

fn remove_prefix(info: &str, fansub: &str) -> Option<String> {
    let mut raw = info.to_string();
    if !fansub.is_empty() {
        raw = build_regex(format!(".{}.", fansub).as_str())
            .replace_all(info, "")
            .to_string();
    }
    let raw_process = build_regex(PREFIX_PATTERN).replace_all(raw.as_str(), "/");
    let mut args = raw_process
        .split("/")
        .filter(|&s| !s.is_empty())
        .collect::<Vec<&str>>();
    if args.len() == 1 {
        args = args[0]
            .split_whitespace()
            .filter(|&s| !s.is_empty())
            .collect::<Vec<&str>>();
    }
    let mut result = raw.clone();
    for arg in args.iter() {
        if (build_regex(r"新番|月?番").is_match(arg) && arg.len() <= 5)
            || build_regex(r"港澳台地区").is_match(arg)
        {
            result = build_regex(format!(".{}.", arg).as_str())
                .replace_all(&result, "")
                .to_string()
        }
    }
    Some(result.trim().to_string())
}

fn parse_season(info: &str) -> Option<(String, i8)> {
    let mut season: i8 = -1;
    let season_re = build_regex(r"S\d{1,2}|Season \d{1,2}|[第].[季期]");
    let name_season = build_regex(r"[\[\]]").replace_all(info, " ");
    let seasons = season_re
        .find_iter(&name_season)
        .map(|m| m.as_str())
        .collect::<Vec<&str>>();
    if seasons.is_empty() {
        return Some((name_season.to_string(), 1));
    }
    let name = season_re.replace_all(&name_season, "").trim().to_string();
    for s in seasons.iter() {
        if build_regex(r"Season|S").is_match(s) {
            season = build_regex(r"Season|S")
                .replace_all(s, "")
                .parse()
                .unwrap_or(-1);
            break;
        } else if build_regex(r"[第 ].*[季期(部分)]|部分").is_match(s) {
            let season_str = &build_regex(r"[第季期 ]").replace_all(s, "").to_string();
            season = season_str
                .parse()
                .unwrap_or_else(|_| match season_str.as_str() {
                    "一" => 1,
                    "二" => 2,
                    "三" => 3,
                    "四" => 4,
                    "五" => 5,
                    "六" => 6,
                    "七" => 7,
                    "八" => 8,
                    "九" => 9,
                    "十" => 10,
                    _ => -1,
                });
            break;
        }
    }
    if name.trim().is_empty() || season == -1 {
        return None;
    }
    Some((name, season))
}

fn parse_title(title: &str) -> (Option<String>, Option<String>, Option<String>) {
    let mut title_zh = None;
    let mut title_en = None;
    let mut title_jp = None;

    let title = build_regex(r"[(（]仅限港澳台地区[）)]").replace_all(title, "");
    let mut split: Vec<&str> = build_regex(r"/|\s{2}|-\s{2}")
        .split(&title)
        .filter(|&s| !s.is_empty())
        .collect();
    if split.len() == 1 {
        if build_regex(r"_{1}").is_match(&title) {
            split = build_regex(r"_").split(&title).collect();
        } else if build_regex(r" - {1}").is_match(&title) {
            split = build_regex("-").split(&title).collect();
        }
    }
    let mut split_result = split.clone();
    if split.len() == 1 {
        let mut split_space = split[0].split_whitespace().collect::<Vec<&str>>();
        split_space = vec![split_space[0], split_space.last().unwrap()];
        for s in split_space.clone().iter() {
            if build_regex(r"^[\u4e00-\u9fa5]{2,}").is_match(s) {
                split_space.retain(|x| x != s);
                split_result.clear();
                split_result.push(s);
                split_result.push(&split_space.join(" "));
                break;
            }
        }
    }
    for s in split.iter() {
        if build_regex(r"[\u4e00-\u9fa5]{2,}").is_match(s) {
            title_zh = Some(s.trim().to_string());
        } else if build_regex(r"[a-zA-Z]{3,}").is_match(s) {
            title_en = Some(s.trim().to_string());
        } else if build_regex(r"[\u0800-\u4e00]{2,}").is_match(s) {
            title_jp = Some(s.trim().to_string());
        }
    }
    (title_zh, title_en, title_jp)
}

pub fn parse(title: &str) -> Result<ParseResult, BoxErr> {
    let title = title.trim().replace("【", "[").replace("】", "]");
    let fansub = parse_fansub(&title).unwrap_or_default();
    let info_split = build_regex(TITLE_PATTERN)
        .captures(&title)
        .ok_or(ParseError::InvalidInput)?;
    let infos: Vec<&str> = (1..=3)
        .map(|i| info_split.get(i).map(|s| s.as_str()).unwrap_or_default())
        .collect();
    let (season_info, episode_info, _) = (infos[0], infos[1], infos[2]);
    let raw_season_info = remove_prefix(season_info, &fansub).unwrap_or_default();
    if raw_season_info.trim().is_empty() {
        return Err(Box::new(ParseError::InvalidInput));
    }
    let (raw_name, season) = parse_season(&raw_season_info).ok_or(ParseError::InvalidSeason)?;
    let (title_zh, title_en, title_jp) = parse_title(&raw_name);
    if title_zh.is_none() && title_en.is_none() && title_jp.is_none() {
        return Err(Box::new(ParseError::InvalidTitle));
    }
    let raw_episode = build_regex(EPISODE_PATTERN)
        .find(episode_info)
        .unwrap()
        .as_str();
    let episode = raw_episode
        .parse::<i16>()
        .map_err(|_| ParseError::InvalidEpisode)?;
    Ok(ParseResult {
        title_zh: title_zh.unwrap_or_default(),
        title_en: title_en.unwrap_or_default(),
        title_jp: title_jp.unwrap_or_default(),
        fansub,
        season,
        episode,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse() {
        let title = "[ANi] 我内心的糟糕念头 第二季 - 13 [1080P][Baha][WEB-DL][AAC AVC][CHT][MP4]";
        let result = parse(title).unwrap();
        assert_eq!(
            result,
            ParseResult {
                title_zh: "我内心的糟糕念头".to_string(),
                title_en: "".to_string(),
                title_jp: "".to_string(),
                fansub: "ANi".to_string(),
                season: 2,
                episode: 13,
            }
        );
    }
}
