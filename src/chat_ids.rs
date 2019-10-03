use std::error::Error;

use std::path::Path;

use rocksdb::{IteratorMode, DB};

use std::convert::TryInto;
use telegram_bot::ChatId;

#[derive(Debug)]
pub struct ChatIDs {
    db: DB,
}

impl ChatIDs {
    pub fn new<T: AsRef<Path>>(path: T) -> Result<Self, Box<dyn Error>> {
        let db = DB::open_default(path)?;
        Ok(Self { db })
    }

    pub fn contains(&self, id: impl Into<i64>) -> bool {
        let key = id.into().to_be_bytes();
        match self.db.get(key) {
            Ok(data) => data.is_some(),
            _ => false,
        }
    }

    pub fn put(&self, id: impl Into<i64>) -> Result<(), Box<dyn Error>> {
        let key = id.into().to_be_bytes();
        self.db.put(&key, b"")?;
        Ok(())
    }

    pub fn remove(&self, id: impl Into<i64>) -> Result<(), Box<dyn Error>> {
        let key = id.into().to_be_bytes();
        self.db.delete(&key)?;
        Ok(())
    }

    pub fn iter(&self) -> impl Iterator<Item = ChatId> + '_ {
        self.db.iterator(IteratorMode::Start).map(|(key, _)| {
            let raw_id = i64::from_be_bytes(
                key.as_ref()
                    .try_into()
                    .expect("BUG: Key with length != 8 bytes"),
            );
            raw_id.into()
        })
    }
}
