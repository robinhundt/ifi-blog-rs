use std::path::{Path};

use rocksdb::{self, IteratorMode, DB};

use teloxide::types::ChatId;

use anyhow::{Result, Context};

#[derive(Debug)]
/// A wrapper around a RocksDB DB instance. Provides
/// functions to insert, remove, check for or iterate
/// over ChatID's persistently stored
pub struct ChatIDs {
    db: DB,
}

impl ChatIDs {
    /// Creates or opens a database at the provided path
    pub fn new<T: AsRef<Path>>(path: T) -> Result<Self> {
        let db = DB::open_default(&path).context("Unable to open DB")?;
        Ok(Self { db })
    }

    /// Checks whether the given ChatID is contained in the database
    pub fn contains(&self, id: &ChatId) -> bool {
        let key = bincode::serialize(id).expect("Bincode serialization failed");
        match self.db.get(key) {
            Ok(data) => data.is_some(),
            _ => false,
        }
    }

    /// Adds a ChatID to the database
    pub fn put(&self, id: &ChatId) -> Result<()> {
        let key = bincode::serialize(id).expect("Bincode serialization failed");
        self.db.put(&key, b"").context("Unable to store ID")?;
        Ok(())
    }

    /// Removes a ChatID from the database
    pub fn remove(&self, id: &ChatId) -> Result<()> {
        let key = bincode::serialize(id).expect("Bincode serialization failed");
        self.db.delete(&key).context("Unable to remove ID")?;
        Ok(())
    }

    /// Provides an Iterator over all ChatID's in the database
    pub fn iter(&self) -> impl Iterator<Item = ChatId> + '_ {
        self.db.iterator(IteratorMode::Start).map(|(key, _)| {
            bincode::deserialize(&key).expect("Bincode deserialization failed")
        })
    }
}