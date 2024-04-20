use polodb_core::{bson::doc, Database};
use serde::{Deserialize, Serialize};
use std::error::Error;
use thiserror::Error;

type BoxErr = Box<dyn Error + Send + Sync>;

pub struct Client {
    db: Database,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Bangumi {
    pub id: u32,
    pub title: String,
    pub weekday: u8,
    pub poster_url: String,
    pub downloaded: Vec<String>,
    pub rss_url: String,
    pub enabled: bool,
    pub not_contains: Vec<String>,
}

#[derive(Error, Debug)]
pub enum DatabaseError {
    #[error("Bangumi not found")]
    BangumiNotFound,
    #[error("Bangumi existed")]
    BangumiExisted,
}

impl Client {
    pub fn new(db_path: &str) -> Result<Self, BoxErr> {
        let db = Database::open_file(db_path)?;
        Ok(Self { db })
    }
    pub fn get_bangumi(&self, id: u32) -> Result<Option<Bangumi>, BoxErr> {
        let bangumi = self
            .db
            .collection::<Bangumi>("bangumi")
            .find_one(doc! { "id": id })?;
        Ok(bangumi)
    }
    pub fn insert_bangumi(&self, bangumi: Bangumi) -> Result<(), BoxErr> {
        if self.get_bangumi(bangumi.id)?.is_some() {
            return Err(Box::new(DatabaseError::BangumiExisted));
        }
        self.db
            .collection::<Bangumi>("bangumi")
            .insert_one(bangumi)?;
        Ok(())
    }
    pub fn delete_bangumi(&self, id: u32) -> Result<(), BoxErr> {
        self.db
            .collection::<Bangumi>("bangumi")
            .delete_one(doc! { "id": id })?;
        Ok(())
    }
    pub fn get_bangumi_all(&self) -> Result<Vec<Bangumi>, BoxErr> {
        let bangumi = self
            .db
            .collection::<Bangumi>("bangumi")
            .find(None)?
            .collect::<polodb_core::Result<Vec<Bangumi>>>()?;
        Ok(bangumi)
    }
    pub fn get_rss_bangumi(&self) -> Result<Vec<Bangumi>, BoxErr> {
        let bangumi = self
            .db
            .collection::<Bangumi>("bangumi")
            .find(doc! { "enabled": true })?
            .collect::<polodb_core::Result<Vec<Bangumi>>>()?;
        Ok(bangumi)
    }
    pub fn add_downloaded(&self, id: u32, hash: &str) -> Result<(), BoxErr> {
        let mut bangumi = self
            .get_bangumi(id)?
            .ok_or(DatabaseError::BangumiNotFound)?;
        if !bangumi.downloaded.contains(&hash.to_string()) {
            bangumi.downloaded.push(hash.to_string());
            self.db.collection::<Bangumi>("bangumi").update_one(
                doc! { "id": id },
                doc! { "$set": { "downloaded": bangumi.downloaded } },
            )?;
        }
        Ok(())
    }
    pub fn is_downloaded(&self, id: u32, hash: &str) -> Result<bool, BoxErr> {
        let bangumi = self
            .get_bangumi(id)?
            .ok_or(DatabaseError::BangumiNotFound)?;
        Ok(bangumi.downloaded.contains(&hash.to_string()))
    }
    pub fn bangumi_exists(&self, id: u32) -> Result<bool, BoxErr> {
        let bangumi = self.get_bangumi(id)?;
        Ok(bangumi.is_some())
    }
    pub fn set_bangumi_enabled(&self, id: u32, enabled: bool) -> Result<(), BoxErr> {
        self.db
            .collection::<Bangumi>("bangumi")
            .update_one(doc! { "id": id }, doc! { "$set": { "enabled": enabled } })?;
        Ok(())
    }
    pub fn set_bangumi_not_contains(
        &self,
        id: u32,
        not_contains: Vec<String>,
    ) -> Result<(), BoxErr> {
        self.db.collection::<Bangumi>("bangumi").update_one(
            doc! { "id": id },
            doc! { "$set": { "not_contains": not_contains } },
        )?;
        Ok(())
    }
}
