use anyhow::Result;
use reqwest::{self, Proxy};
use std::{io::Write, path::Path};

pub async fn download_file(url: &str, save_path: &str, proxy: Option<Proxy>) -> Result<()> {
    let mut client = reqwest::Client::new();
    if let Some(proxy) = proxy {
        client = reqwest::Client::builder().proxy(proxy).build()?;
    }
    let mut file = std::fs::File::create(save_path)?;
    let mut resp = client.get(url).send().await?;
    let mut content = Vec::new();
    while let Some(chunk) = resp.chunk().await? {
        content.extend_from_slice(&chunk);
    }
    file.write_all(&content)?;
    Ok(())
}

pub async fn ensure_dir(path: &str) -> Result<()> {
    if !std::path::Path::new(path).exists() {
        std::fs::create_dir_all(path).unwrap();
    }
    Ok(())
}

pub async fn delete_file(path: &str) -> Result<()> {
    if std::path::Path::new(path).exists() {
        std::fs::remove_file(path)?;
    }
    Ok(())
}

pub fn file_stem(file_name: &str) -> Option<&str> {
    Path::new(file_name).file_stem()?.to_str()
}

pub fn file_extension(file_name: &str) -> Option<&str> {
    Path::new(file_name).extension()?.to_str()
}
