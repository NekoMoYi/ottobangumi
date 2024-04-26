use dotenv::dotenv;
use lava_torrent::torrent::v1::Torrent;
use ottobangumi::*;
use std::env;
use std::sync::Arc;
use teloxide::requests::Requester;

type BoxErr = Box<dyn std::error::Error + Send + Sync>;

#[tokio::main]
async fn main() -> Result<(), BoxErr> {
    dotenv().ok();
    env_logger::init();
    let mut proxy: Option<reqwest::Proxy> = None;
    if let Some(proxy_url) = env::var("PROXY_URL").ok() {
        proxy = Some(reqwest::Proxy::all(&proxy_url)?);
        env::set_var("TELOXIDE_PROXY", proxy_url);
    }
    let rss_interval: u32 = env::var("RSS_INTERVAL")
        .unwrap_or("3600".to_string())
        .parse()
        .unwrap();
    let db_path = env::var("DATABASE").unwrap_or("otto.db".to_string());
    let db = Arc::new(database::Client::new(&db_path)?);
    let config = Arc::new(bot::Config {
        user_ids: env::var("USER_IDS")
            .unwrap_or_default()
            .split(',')
            .map(|id| id.parse().unwrap())
            .collect(),
        proxy,
        not_contains: env::var("NOT_CONTAINS")
            .unwrap_or_default()
            .split(',')
            .map(|s| s.to_string())
            .collect(),
        tmp_dir: env::var("TMP_DIR").unwrap_or("tmp".to_string()),
        lib_dir: env::var("LIB_DIR").unwrap(),
    });
    let bot = bot::MyBot::new(config.clone(), db.clone()).await?;
    let tg_bot = bot.tg.clone();
    let (bot_handle, _) = bot.spawn();

    tokio::spawn(async move {
        loop {
            let bangumis = db.get_rss_bangumi();
            match bangumis {
                Ok(bangumis) => {
                    if bangumis.is_empty() {
                        log::info!("no rss to update");
                    }
                    for b in bangumis {
                        log::info!("update rss: {} - {}", b.id, b.title);
                        if let Err(e) =
                            update_rss(b, db.clone(), config.clone(), tg_bot.clone()).await
                        {
                            log::error!("update rss error: {:?}", e);
                        }
                    }
                }
                Err(e) => {
                    log::error!("database error: {:?}", e);
                }
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(rss_interval as u64)).await;
        }
    });

    if let Err(e) = tokio::try_join!(bot_handle) {
        log::error!("bot error: {:?}", e);
    }
    Ok(())
}

async fn update_rss(
    b: database::Bangumi,
    db: Arc<database::Client>,
    cfg: Arc<bot::Config>,
    tg: Arc<teloxide::Bot>,
) -> Result<(), BoxErr> {
    let rss = mikan::MikanRss::from_url(&b.rss_url)
        .set_proxy(cfg.proxy.clone())?
        .fetch()
        .await?;
    for ep in rss.items.iter().rev() {
        if cfg.not_contains.iter().any(|s| ep.title.contains(s))
            || ep.torrent_hash.is_empty()
            || b.downloaded.contains(&ep.torrent_hash)
        {
            continue;
        }
        log::info!("starting download: {:?}", ep);
        let ep_info = title_parser::parse(&ep.title)?;
        let torrent_url = ep.torrent_url.as_str();
        let torrent_name = torrent_url.split('/').last();
        if torrent_name.is_none() {
            log::error!("torrent name is none: {}", torrent_url);
            continue;
        }
        let torrent_path = format!("{}/{}", cfg.tmp_dir, torrent_name.unwrap());
        utils::download_file(torrent_url, &torrent_path, cfg.proxy.clone()).await?;
        let save_dir = format!("{}/{}/Season {}", cfg.lib_dir, b.title, ep_info.season);
        let save_name = format!("{} S{:02}E{:02}", b.title, ep_info.season, ep_info.episode);
        let torrent_hash = Torrent::read_from_file(&torrent_path)?.info_hash();
        let downloader = downloader::QbitDownloader::new().await?;
        downloader
            .download_by_torrent_to(&torrent_path, &torrent_hash, &save_dir, &save_name)
            .await?;
        db.add_downloaded(b.id, &torrent_hash)?;
        for chat_id in cfg.user_ids.iter() {
            tg.send_message(
                teloxide::types::ChatId(*chat_id as i64),
                format!("{} updated.", save_name),
            )
            .await?;
        }
    }
    Ok(())
}
