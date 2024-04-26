use crate::{database, mikan, utils};
use anyhow::Result;
use std::sync::Arc;
use teloxide::dispatching::DefaultKey;
use teloxide::types::InputFile;
use teloxide::{prelude::*, utils::command::BotCommands};

#[derive(BotCommands, Clone, Debug)]
#[command(
    rename_rule = "lowercase",
    description = "These commands are supported:"
)]
pub enum Command {
    #[command(description = "display this text.")]
    Help,
    #[command(description = "list all bangumi.")]
    List,
    #[command(description = "get bangumi info.\nUsage: /info <id>")]
    Info(u32),
    #[command(description = "add a bangumi.\nUsage: /add <rss_url>")]
    Add(String),
    #[command(description = "remove a bangumi.\nUsage: /remove <id>")]
    Remove(u32),
    #[command(description = "enable rss.\nUsage: /enable <id>")]
    Enable(u32),
    #[command(description = "disable rss.\nUsage: /disable <id>")]
    Disable(u32),
    #[command(
        description = "set not contains words by id.\nUsage: /nc <id> <word1,word2,...>/none",
        parse_with = "split",
        rename = "nc"
    )]
    NotContains(u32, String),
}

pub struct Config {
    pub proxy: Option<reqwest::Proxy>,
    pub user_ids: Vec<u64>,
    pub not_contains: Vec<String>,
    pub tmp_dir: String,
    pub lib_dir: String,
}

pub struct MyBot {
    pub tg: Arc<Bot>,
    pub dispatcher: Dispatcher<Arc<Bot>, anyhow::Error, DefaultKey>,
}

impl MyBot {
    pub async fn new(config: Arc<Config>, db: Arc<database::Client>) -> Result<Self> {
        let tg = Arc::new(Bot::from_env());
        tg.set_my_commands(Command::bot_commands()).await?;

        let handler = Update::filter_message().branch(
            dptree::filter(|msg: Message, config: Arc<Config>| {
                msg.from()
                    .map(|user| config.user_ids.contains(&user.id.0))
                    .unwrap_or_default()
            })
            .filter_command::<Command>()
            .endpoint(bot_handler),
        );

        let dispatcher = Dispatcher::builder(tg.clone(), handler)
            .dependencies(dptree::deps![config.clone(), db.clone()])
            .default_handler(|upd| async move {
                log::info!("unhandled update: {:?}", upd);
            })
            .error_handler(LoggingErrorHandler::with_custom_text(
                "An error has occurred in the dispatcher",
            ))
            .build();

        let bot = MyBot {
            tg: tg.clone(),
            dispatcher,
        };
        Ok(bot)
    }

    pub fn spawn(
        mut self,
    ) -> (
        tokio::task::JoinHandle<()>,
        teloxide::dispatching::ShutdownToken,
    ) {
        let shutdown_token = self.dispatcher.shutdown_token();
        (
            tokio::spawn(async move { self.dispatcher.dispatch().await }),
            shutdown_token,
        )
    }
}

pub async fn bot_handler(
    msg: Message,
    bot: Arc<Bot>,
    cmd: Command,
    config: Arc<Config>,
    db: Arc<database::Client>,
) -> Result<()> {
    let chat_id = msg.chat.id;
    log::info!("ChatId: {:?}, Command: {:?}", chat_id, cmd);
    let handler = BotHandler::new(bot.clone(), chat_id, config, db);
    match cmd {
        Command::Help => handler.bot_help().await?,
        Command::List => handler.bangumi_list().await?,
        Command::Add(url) => handler.bangumi_add(url).await?,
        Command::Remove(id) => handler.bangumi_remove(id).await?,
        Command::Info(id) => handler.bangumi_info(id).await?,
        Command::Enable(id) => handler.bangumi_enable(id).await?,
        Command::Disable(id) => handler.bangumi_disable(id).await?,
        Command::NotContains(id, words) => handler.bangumi_not_contains(id, words).await?,
    };
    Ok(())
}

pub struct BotHandler {
    pub bot: Arc<Bot>,
    pub chat_id: ChatId,
    pub config: Arc<Config>,
    pub db: Arc<database::Client>,
}

impl BotHandler {
    pub fn new(
        bot: Arc<Bot>,
        chat_id: ChatId,
        config: Arc<Config>,
        db: Arc<database::Client>,
    ) -> Self {
        BotHandler {
            bot,
            chat_id,
            config,
            db,
        }
    }
    pub async fn bot_help(&self) -> Result<()> {
        self.bot
            .send_message(self.chat_id, Command::descriptions().to_string())
            .await?;
        Ok(())
    }
    pub async fn bangumi_list(&self) -> Result<()> {
        let bangumi = match self.db.get_bangumi_all() {
            Ok(b) => b,
            Err(e) => {
                log::error!("database error: {:?}", e);
                self.bot.send_message(self.chat_id, "Failed.").await?;
                return Ok(());
            }
        };
        if bangumi.is_empty() {
            self.bot
                .send_message(self.chat_id, "no bangumi found.")
                .await?;
        } else {
            let text = bangumi
                .iter()
                .map(|b| format!("{}\n{}\n", b.id, b.title))
                .collect::<Vec<String>>()
                .join("\n");
            self.bot.send_message(self.chat_id, text).await?;
        }
        Ok(())
    }
    pub async fn bangumi_add(&self, url: String) -> Result<()> {
        if url.is_empty() {
            self.bot
                .send_message(self.chat_id, "rss url required.")
                .await?;
            return Ok(());
        }
        self.bot.send_message(self.chat_id, "fetching...").await?;
        match mikan::MikanParser::new()
            .set_proxy(self.config.proxy.clone())?
            .from_rss_url(&url)
            .await
        {
            Ok(b) => {
                let bangumi = database::Bangumi {
                    id: b.id,
                    title: b.title.clone(),
                    weekday: b.weekday,
                    poster_url: b.poster_url.clone(),
                    downloaded: vec![],
                    rss_url: url,
                    enabled: true,
                    not_contains: self.config.not_contains.clone(),
                };
                if let Ok(true) = self.db.bangumi_exists(b.id) {
                    self.bot
                        .send_message(self.chat_id, "Bangumi already exists.")
                        .await?;
                    return Ok(());
                }
                match self.db.insert_bangumi(bangumi) {
                    Ok(_) => {
                        utils::ensure_dir(&self.config.tmp_dir).await?;
                        let poster_path = format!(
                            "{}/{}",
                            self.config.tmp_dir,
                            b.poster_url.split('/').last().unwrap()
                        );
                        utils::download_file(
                            &b.poster_url,
                            &poster_path,
                            self.config.proxy.clone(),
                        )
                        .await?;
                        let poster_file = InputFile::file(poster_path);
                        self.bot.send_photo(self.chat_id, poster_file).await?;
                        self.bot
                            .send_message(self.chat_id, format!("{}\n{}", b.id, b.title))
                            .await?;
                        self.bot.send_message(self.chat_id, "Success.").await?;
                    }
                    Err(e) => {
                        log::error!("database error: {:?}", e);
                        self.bot.send_message(self.chat_id, "Failed.").await?;
                    }
                }
            }
            Err(e) => {
                log::error!("RSS error: {:?}", e);
                self.bot.send_message(self.chat_id, "Failed.").await?;
            }
        }
        Ok(())
    }
    pub async fn bangumi_remove(&self, id: u32) -> Result<()> {
        match self.db.delete_bangumi(id) {
            Ok(_) => {
                self.bot.send_message(self.chat_id, "Success.").await?;
            }
            Err(e) => {
                log::error!("database error: {:?}", e);
                self.bot.send_message(self.chat_id, "Failed.").await?;
            }
        }
        Ok(())
    }
    pub async fn bangumi_info(&self, id: u32) -> Result<()> {
        match self.db.get_bangumi(id) {
            Ok(b) => {
                if let Some(b) = b {
                    let text = format!(
                        "id: {}\ntitle: {}\nweekday: {}\nposter: {}\nurl: {}\nenabled: {}\nnot contains: {:?}\ndownloaded: {}",
                        b.id, b.title, b.weekday, b.poster_url, b.rss_url, b.enabled, b.not_contains, b.downloaded.len()
                    );
                    self.bot.send_message(self.chat_id, text).await?;
                } else {
                    self.bot
                        .send_message(self.chat_id, "Bangumi not found.")
                        .await?;
                }
            }
            Err(e) => {
                log::error!("database error: {:?}", e);
                self.bot.send_message(self.chat_id, "Failed.").await?;
            }
        }
        Ok(())
    }
    pub async fn bangumi_enable(&self, id: u32) -> Result<()> {
        match self.db.set_bangumi_enabled(id, true) {
            Ok(_) => {
                self.bot.send_message(self.chat_id, "Success.").await?;
            }
            Err(e) => {
                log::error!("database error: {:?}", e);
                self.bot.send_message(self.chat_id, "Failed.").await?;
            }
        }
        Ok(())
    }
    pub async fn bangumi_disable(&self, id: u32) -> Result<()> {
        match self.db.set_bangumi_enabled(id, false) {
            Ok(_) => {
                self.bot.send_message(self.chat_id, "Success.").await?;
            }
            Err(e) => {
                log::error!("database error: {:?}", e);
                self.bot.send_message(self.chat_id, "Failed.").await?;
            }
        }
        Ok(())
    }
    pub async fn bangumi_not_contains(&self, id: u32, words: String) -> Result<()> {
        let mut words = words.split(',').map(|w| w.to_string()).collect();
        if words == vec!["none"] {
            words = vec![];
        }
        match self.db.set_bangumi_not_contains(id, words) {
            Ok(_) => {
                self.bot.send_message(self.chat_id, "Success.").await?;
            }
            Err(e) => {
                log::error!("database error: {:?}", e);
                self.bot.send_message(self.chat_id, "Failed.").await?;
            }
        }
        Ok(())
    }
}
