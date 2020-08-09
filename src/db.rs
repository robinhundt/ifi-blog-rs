use crate::util::display_iterable;
use anyhow::{Context, Result};
use refinery::config::{Config, ConfigDbType};
use sqlx::{query_as, FromRow, SqlitePool};
use std::env;
use teloxide::types::ChatId;

#[derive(FromRow, Debug)]
pub struct Chat {
    id: i64,
    chat_id: Option<i64>,
    channel_name: Option<String>,
}

impl Chat {
    pub async fn insert(pool: &SqlitePool, chat_id: ChatId) -> Result<Self> {
        let mut conn = pool.acquire().await?;
        let (chat_id, channel_name) = split_chat_id(chat_id);
        sqlx::query!(
            r#"
            insert into chats (chat_id, channel_name)
            values ( ?, ?)
            "#,
            chat_id,
            channel_name
        )
        .execute(&mut conn)
        .await?;
        let chat = sqlx::query_as!(Self, "select * from chats where id=last_insert_rowid()")
            .fetch_one(&mut conn)
            .await?;
        log::info!("Added chat {:?} to db", &chat);
        Ok(chat)
    }

    pub async fn delete(pool: &SqlitePool, chat_id: ChatId) -> Result<()> {
        let (chat_id, channel_name) = split_chat_id(chat_id);
        sqlx::query!(
            r#"
            delete from chats
            where chat_id = ? or channel_name = ?
            "#,
            chat_id,
            channel_name
        )
        .execute(pool)
        .await?;
        log::info!("Removed chat {:?} from db", &chat_id);
        Ok(())
    }

    pub async fn contains(pool: &SqlitePool, chat_id: ChatId) -> Result<bool> {
        let (chat_id, channel_name) = split_chat_id(chat_id);
        // TODO: For some reason when using the macro version, the compilation is stuck in a loop
        // should be tasted with a later version
        let exists = sqlx::query(
            r#"
            select id from chats where chat_id = ? or channel_name = ?
            "#,
        )
        .bind(chat_id)
        .bind(channel_name)
        .fetch_optional(pool)
        .await?;
        Ok(exists.is_some())
        // Ok(false)
    }

    pub async fn list(pool: &SqlitePool) -> Result<Vec<Self>> {
        let chats = query_as!(
            Chat,
            r#"
            select * from chats
            "#
        )
        .fetch_all(pool)
        .await?;
        Ok(chats)
    }

    pub fn get_chat_id(&self) -> ChatId {
        if let Some(id) = self.chat_id {
            id.into()
        } else if let Some(name) = &self.channel_name {
            name.clone().into()
        } else {
            panic!("Chat with neither chat_id nor channel_name")
        }
    }
}

mod embedded {
    use refinery::embed_migrations;

    embed_migrations!("migrations");
}

pub fn run_migrations() -> Result<()> {
    let db_url = env::var("DATABASE_URL").context("`DATABASE_URL` must be set")?;
    let db_url = db_url
        .strip_prefix("sqlite:")
        .context("`DATABASE_URL` must have `sqlite:` prefix")?;
    let mut config = Config::new(ConfigDbType::Sqlite).set_db_path(&db_url);
    log::info!("Applying migrations");
    let report = embedded::migrations::runner().run(&mut config)?;
    log::info!(
        "Applied migrations:\n {}---------",
        display_iterable(report.applied_migrations())
    );
    Ok(())
}

fn split_chat_id(id: ChatId) -> (Option<i64>, Option<String>) {
    match id {
        ChatId::Id(id) => (Some(id), None),
        ChatId::ChannelUsername(name) => (None, Some(name)),
        _ => panic!("Unsupported chat id"),
    }
}
