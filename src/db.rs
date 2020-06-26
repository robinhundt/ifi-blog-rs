use anyhow::{Result, Context};
use tokio_postgres::{NoTls};
use crate::util::display_iterable;
use sqlx::{FromRow, PgPool, query_as};
use teloxide::types::ChatId;


#[derive(FromRow, Debug)]
pub struct Chat {
    id: i32,
    chat_id: Option<i64>,
    channel_name: Option<String>
}

impl Chat {
    pub async fn add(mut pool: &PgPool, chat_id: ChatId) -> Result<Self> {
        let (chat_id, channel_name) = split_chat_id(chat_id);
        let chat = sqlx::query_as!(
            Self,
            r#"
            insert into chats (chat_id, channel_name)
            values ( $1, $2)
            returning *
            "#,
            chat_id, channel_name
        ).fetch_one(&mut pool).await?;
        Ok(chat)
    }

    pub async fn remove(mut pool: &PgPool, chat_id: ChatId) -> Result<()> {
        let (chat_id, channel_name) = split_chat_id(chat_id);
        sqlx::query!(
            r#"
            delete from chats
            where chat_id = $1 or channel_name = $2
            "#,
            chat_id, channel_name
        ).execute(&mut pool).await?;
        Ok(())
    }

    pub async fn list(mut pool: &PgPool) -> Result<Vec<Self>> {
        let chats = query_as!(
            Chat,
            r#"
            select * from chats
            "#
        ).fetch_all(&mut pool).await?;
        Ok(chats)
    }
}


mod embedded {
    use refinery::embed_migrations;

    embed_migrations!("migrations");
}

pub async fn run_migrations() -> Result<()> {
    let (mut client, conn) = tokio_postgres::connect("host=localhost user=postgres password=password", NoTls).await?;
    tokio::spawn(async move {
        if let Err(e) = conn.await {
            eprintln!("connection error: {}", e);
        }
    });
    log::info!("Applying migrations");
    let report = embedded::migrations::runner().run_async(&mut client).await?;
    log::info!("Applied migrations:\n {}---------",
    display_iterable(report.applied_migrations()));
    Ok(())
}


fn split_chat_id(id: ChatId) -> (Option<i64>, Option<String>) {
    match id {
        ChatId::Id(id) => (Some(id), None),
        ChatId::ChannelUsername(name) => (None, Some(name)),
    }
}