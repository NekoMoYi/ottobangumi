use qbit_api_rs::{self, types::torrents::FilesResponseItem};
use thiserror::Error;
use crate::utils::{file_extension, file_stem};

type BoxErr = Box<dyn std::error::Error + Send + Sync>;

#[derive(Error, Debug)]
pub enum DownloaderError {
    #[error("Failed to initialize downloader")]
    InitError,
    #[error("No file to rename")]
    NoFileToRename,
    #[error("Failed to parse file name {name}")]
    FileNameError { name: String },
}

pub struct QbitDownloader {
    pub client: qbit_api_rs::client::QbitClient,
}

impl QbitDownloader {
    pub async fn new() -> Result<Self, BoxErr> {
        let client = qbit_api_rs::client::QbitClient::new_from_env()?;
        client.auth_login().await?;
        Ok(QbitDownloader { client })
    }
    pub async fn download(&self, urls: Vec<String>) -> Result<(), BoxErr> {
        self.client.torrents_add_by_url(&urls).await?;
        Ok(())
    }
    pub async fn download_by_torrent_file(
        &self,
        file_path: &str,
    ) -> Result<(), BoxErr> {
        self.client.torrents_add_by_file(&[file_path]).await?;
        Ok(())
    }
    pub fn magnet_to_hash(magnet: &str) -> String {
        let hash = magnet.trim().replace("magnet:?xt=urn:btih:", "");
        hash.split('&').collect::<Vec<&str>>()[0].to_string()
    }
    pub async fn torrent_files(
        &self,
        hash: &str,
    ) -> Result<Vec<FilesResponseItem>, BoxErr> {
        let files = self.client.torrents_files(hash, None).await?;
        Ok(files)
    }
    pub async fn move_files(
        &self,
        hash: &str,
        save_dir: &str,
        save_name: &str,
    ) -> Result<(), BoxErr> {
        self.client.torrents_set_location(&[hash], save_dir).await?;
        let files = self.torrent_files(&hash).await?;
        let mut to_rename: Vec<String> = Vec::new();
        if files.len() == 1 {
            to_rename.push(files[0].name.clone());
        } else {
            let file = match files.iter().max_by_key(|f| f.size) {
                Some(f) => f,
                None => return Err(Box::new(DownloaderError::NoFileToRename)),
            };
            let name = file_stem(&file.name).ok_or(DownloaderError::FileNameError {
                name: file.name.clone(),
            })?;
            to_rename.push(file.name.clone());
            for f in files.iter() {
                if f.name != file.name && f.name.starts_with(name) {
                    to_rename.push(f.name.clone());
                }
            }
        }
        for f in to_rename.iter() {
            let ext = file_extension(f).ok_or(DownloaderError::FileNameError {
                name: f.clone(),
            })?;
            let new_name = format!("{}.{}", save_name, ext);
            self.client.torrents_rename_file(hash, f, &new_name).await?;
        }
        self.client.torernts_rename(hash, save_name).await?;
        Ok(())
    }
    pub async fn download_to(
        &self,
        url: &str,
        save_dir: &str,
        save_name: &str,
    ) -> Result<(), BoxErr> {
        let hash = QbitDownloader::magnet_to_hash(url);
        self.client.torrents_add_by_url(&[url.to_string()]).await?;
        self.move_files(&hash, save_dir, save_name).await?;
        Ok(())
    }
    pub async fn download_by_torrent_to(
        &self,
        file_path: &str,
        hash: &str,
        save_dir: &str,
        save_name: &str,
    ) -> Result<(), BoxErr> {
        self.client.torrents_add_by_file(&[file_path]).await?;
        // wait for the torrent to be added
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        self.move_files(hash, save_dir, save_name).await?;
        Ok(())
    }
}
