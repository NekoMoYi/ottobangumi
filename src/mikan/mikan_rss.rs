type BoxErr = Box<dyn std::error::Error + Send + Sync>;

#[derive(Debug)]
pub struct MikanRss {
    pub url: String,
    client: reqwest::Client,
}

#[derive(Debug)]
pub struct RssBangumi {
    pub title: String,
    pub link: String,
    pub description: String,
    pub items: Vec<RssEpisode>,
}

#[derive(Debug)]
pub struct RssEpisode {
    pub title: String,
    pub link: String,
    pub description: String,
    pub torrent_url: String,
    pub torrent_hash: String,
}

impl From<rss::Channel> for RssBangumi {
    fn from(channel: rss::Channel) -> Self {
        let items = channel
            .items
            .iter()
            .map(|item| {
                let i = item.clone();
                let enclosure = i.enclosure.unwrap_or(rss::Enclosure::default());
                let torrent_url = enclosure.url.to_string();
                let torrent_name = torrent_url.split('/').last().unwrap_or("").to_string();
                let torrent_hash = torrent_name.split('.').next().unwrap_or("").to_string();
                RssEpisode {
                    title: item.title().unwrap_or("").to_string(),
                    link: item.link().unwrap_or("").to_string(),
                    description: item.description().unwrap_or("").to_string(),
                    torrent_url,
                    torrent_hash,
                }
            })
            .collect();
        Self {
            title: channel.title().replace("Mikan Project - ", "").to_string(),
            link: channel.link().to_string(),
            description: channel
                .description()
                .replace("Mikan Project - ", "")
                .to_string(),
            items,
        }
    }
}

impl MikanRss {
    pub fn from_url(url: &str) -> Self {
        Self {
            url: url.to_string(),
            client: reqwest::Client::new(),
        }
    }
    pub fn set_proxy(&mut self, proxy: Option<reqwest::Proxy>) -> Result<&Self, reqwest::Error>{
        self.client = match proxy {
            Some(p) => reqwest::Client::builder().proxy(p).build()?,
            None => reqwest::Client::new(),
        };
        Ok(self)
    }
    pub async fn fetch(&self) -> Result<RssBangumi, BoxErr> {
        let body = self.client.get(&self.url).send().await?.text().await?;
        let channel = rss::Channel::read_from(body.as_bytes())?;
        Ok(RssBangumi::from(channel))
    }
}
