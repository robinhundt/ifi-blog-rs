use std::path::{Path, PathBuf};

use rocksdb::{self, IteratorMode, DB};

use std::convert::TryInto;
use telegram_bot::ChatId;

use snafu::{ResultExt, Snafu};

#[derive(Debug)]
/// A wrapp around a RocksDB DB instance. Provides
/// functions to insert, remove, check for or iterate
/// over ChatID's persistently stored
pub struct ChatIDs {
    db: DB,
}

impl ChatIDs {
    /// Creates or opens a database at the provided path
    pub fn new<T: AsRef<Path>>(path: T) -> Result<Self, DbError> {
        let db = DB::open_default(&path).context(OpenDefault {
            path: path.as_ref().to_path_buf(),
        })?;
        Ok(Self { db })
    }

    /// Checks whether the given ChatID is contained in the database
    pub fn contains(&self, id: ChatId) -> bool {
        let key = i64::from(id).to_be_bytes();
        match self.db.get(key) {
            Ok(data) => data.is_some(),
            _ => false,
        }
    }

    /// Adds a ChatID to the database
    pub fn put(&self, id: ChatId) -> Result<(), DbError> {
        let key = i64::from(id).to_be_bytes();
        self.db.put(&key, b"").context(Put { id })?;
        Ok(())
    }

    /// Removes a ChatID from the database
    pub fn remove(&self, id: ChatId) -> Result<(), DbError> {
        let key = i64::from(id).to_be_bytes();
        self.db.delete(&key).context(Remove { id })?;
        Ok(())
    }

    /// Provides an Iterator over all ChatID's in the database
    pub fn iter(&self) -> impl Iterator<Item = ChatId> + '_ {
        self.db.iterator(IteratorMode::Start).map(|(key, _)| {
            let raw_id = i64::from_be_bytes(
                key.as_ref()
                    .try_into()
                    .expect("BUG: Key with length != 8 bytes in database"),
            );
            raw_id.into()
        })
    }
}

#[derive(Debug, Snafu)]
pub enum DbError {
    #[snafu(display("Unable to create / open database at {}: {:?}", path.display(), source))]
    OpenDefault {
        source: rocksdb::Error,
        path: PathBuf,
    },
    #[snafu(display("Unable to inser id {}: {:?}", id, source))]
    Put { source: rocksdb::Error, id: ChatId },
    #[snafu(display("Unable to inser id {}: {:?}", id, source))]
    Remove { source: rocksdb::Error, id: ChatId },
}
