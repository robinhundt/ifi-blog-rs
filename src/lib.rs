#[macro_use]
extern crate anyhow;

use crate::db::Chat;
use anyhow::{Context, Result};
use futures::select;
use futures_util::core_reexport::str::FromStr;
use futures_util::FutureExt;
use rss::{Channel, Item};
use sqlx::SqlitePool;
use std::collections::HashSet;
use std::env;
use std::iter::FromIterator;
use std::ops::Not;
use std::sync::Arc;
use teloxide::prelude::*;
use teloxide::types::{ChatId, ParseMode};
use teloxide::utils::command::BotCommand;
use tokio::fs::File;
use tokio::io::AsyncReadExt;
use tokio::sync::Mutex;
use tokio::time::{delay_for, Duration};

pub mod db;
pub(crate) mod util;

struct BotContext {
    rss_url: String,
    db_pool: SqlitePool,
    latest_post: Mutex<Option<Item>>,
    admins: HashSet<String>,
}

#[derive(BotCommand)]
#[command(
    rename = "lowercase",
    description = "IFIBlogBot by @robinhundt. \
    Admins can start/stop updates for channels via /start @channelname ."
)]
enum Command {
    // TODO: The parse_split is necessary due to bug
    // https://github.com/teloxide/teloxide-macros/issues/8
    #[command(description = "Start the automatic blog updates", parse_with = "split")]
    Start(OptChatId),
    #[command(description = "Stop the automatic blog updates", parse_with = "split")]
    Stop(OptChatId),
    #[command(description = "Display an about message")]
    About,
    #[command(
        description = "Check whether you're currently subscribed",
        parse_with = "split"
    )]
    Check(OptChatId),
    #[command(description = "Fetch the latest blog post")]
    Latest,
    #[command(description = "Display this help message")]
    Help,
}

#[derive(Debug, Clone)]
struct OptChatId(Option<ChatId>);

impl BotContext {
    async fn new(rss_url: &str) -> Result<Self> {
        let mut buf = String::new();
        let mut admin_file = File::open("admin_list.txt").await?;
        admin_file.read_to_string(&mut buf).await?;
        let admins = HashSet::from_iter(buf.split_whitespace().map(|el| el.to_owned()));

        let db_pool =
            SqlitePool::connect(&env::var("DATABASE_URL").context("`DATABASE_URL` must be set")?)
                .await?;
        Ok(Self {
            rss_url: rss_url.into(),
            db_pool,
            latest_post: Default::default(),
            admins,
        })
    }
}

pub async fn run(rss_url: &str) -> Result<()> {
    log::info!("Starting IfiBlogBot");
    db::run_migrations()?;

    let bot = Bot::from_env();
    let bot_ctx = Arc::new(BotContext::new(rss_url).await?);
    let bot_ctx2 = bot_ctx.clone();
    let dispatcher =
        Dispatcher::new(bot.clone()).messages_handler(|rx| handle_commands(rx, bot_ctx2));
    let mut dispatch = Box::pin(dispatcher.dispatch()).fuse();
    return select! {
         _ = dispatch => Err(anyhow!("Dispatcher stopped working")),
         _ = run_recurring_tasks(bot, bot_ctx).fuse() => Err(anyhow!("Recurring updates stopped working"))
    };
}

async fn handle_commands(rx: DispatcherHandlerRx<Message>, bot_ctx: Arc<BotContext>) {
    let bot_name = env::var("BOT_NAME").expect("Env var 'BOT_NAME' must be set");
    let bot_ctx = &bot_ctx;
    rx.commands::<Command, &str>(&bot_name)
        .for_each_concurrent(None, |(cx, command)| async move {
            action(cx, bot_ctx, command).await.log_on_error().await;
        })
        .await;
}

async fn action(cx: UpdateWithCx<Message>, bot_ctx: &BotContext, command: Command) -> Result<()> {
    match command {
        Command::Help => cx.answer(Command::descriptions()).send().await.map(drop)?,
        Command::Start(chat_id) => start(&cx, bot_ctx, chat_id).await?,
        Command::Stop(chat_id) => stop(&cx, bot_ctx, chat_id).await?,
        Command::Check(chat_id) => check(&cx, bot_ctx, chat_id).await?,
        Command::Latest => latest(&cx, bot_ctx).await?,
        Command::About => about(&cx).await?,
    };

    Ok(())
}

async fn start(cx: &UpdateWithCx<Message>, bot_ctx: &BotContext, chat_id: OptChatId) -> Result<()> {
    let (id, needs_admin) = match chat_id {
        OptChatId(None) => (cx.chat_id().into(), false),
        OptChatId(Some(id)) => (id, true),
    };

    if needs_admin {
        check_perm(cx, bot_ctx).await?;
    }

    let result = Chat::insert(&bot_ctx.db_pool, id.clone()).await;
    if matches!(result, Err(_)) {
        cx.bot
            .send_message(
                id.clone(),
                "Unable to subscribe to the blog. Possibly you're already subscribed.",
            )
            .send()
            .await?;
        result.with_context(|| format!("Unable to store ID ({}) in DB", &id))?;
    }
    cx.bot
        .send_message(
            id.clone(),
            "You are now subscribed to the IfIBlog. The latest post is:",
        )
        .send()
        .await?;
    let post = format_post(&fetch_latest_post(bot_ctx)?);
    cx.bot
        .send_message(id, post)
        .parse_mode(ParseMode::HTML)
        .send()
        .await?;
    Ok(())
}

async fn stop(cx: &UpdateWithCx<Message>, bot_ctx: &BotContext, chat_id: OptChatId) -> Result<()> {
    let (id, needs_admin) = match chat_id {
        OptChatId(None) => (cx.chat_id().into(), false),
        OptChatId(Some(id)) => (id, true),
    };

    if needs_admin {
        check_perm(cx, bot_ctx).await?;
    }
    Chat::delete(&bot_ctx.db_pool, id.clone())
        .await
        .with_context(|| format!("Unable to remove ID ({}) from DB", &id))?;
    cx.bot
        .send_message(id, "You are now unsubscribed from the IfIBlog.")
        .send()
        .await?;
    Ok(())
}

async fn check(cx: &UpdateWithCx<Message>, bot_ctx: &BotContext, chat_id: OptChatId) -> Result<()> {
    let (id, needs_admin) = match chat_id {
        OptChatId(None) => (cx.chat_id().into(), false),
        OptChatId(Some(id)) => (id, true),
    };

    if needs_admin {
        check_perm(cx, bot_ctx).await?;
    }
    let pronoun = if needs_admin {
        format!("{} is", id)
    } else {
        "You're".to_owned()
    };
    let reply = if Chat::contains(&bot_ctx.db_pool, id).await? {
        format!(
            "{} currently subscribed to the blog. Enter /stop to unsubscribe.",
            pronoun
        )
    } else {
        format!(
            "{} currently not subscribed to the blog. Enter /start to subscribe.",
            pronoun
        )
    };
    cx.reply_to(reply).send().await?;
    Ok(())
}

async fn latest(cx: &UpdateWithCx<Message>, bot_ctx: &BotContext) -> Result<()> {
    let post = fetch_latest_post(bot_ctx)?;
    let post_text = format_post(&post);
    cx.reply_to(post_text)
        .parse_mode(ParseMode::HTML)
        .send()
        .await?;
    Ok(())
}

async fn about(cx: &UpdateWithCx<Message>) -> Result<()> {
    let reply = cx.reply_to(
        "Hi, I'm a small bot written by @robinhundt, that serves you the newest news \
        from the [CS deanery blog](https://blog.stud.uni-goettingen.de/informatikstudiendekanat/)\n\
        You can look at my source code on [gitlab](https://gitlab.gwdg.de/robinwilliam.hundt/ifi-blog-rs).\n\
        I'm written in Rust with bleeding edge features! :D");
    reply.parse_mode(ParseMode::MarkdownV2).send().await?;
    Ok(())
}

async fn run_recurring_tasks(bot: Bot, ctx: Arc<BotContext>) {
    log::info!("Starting recurring tasks loop...");
    log::info!(
        "Subscribed chats: {:?}",
        Chat::list(&ctx.db_pool).await.expect("Unable to retrieve subscribed chats")
    );
    loop {
        let ret = send_updates_to_subscribers(bot.clone(), &ctx).await;
        if let Err(err) = ret {
            log::error!("{}", err);
        }
        delay_for(Duration::from_secs(600)).await;
    }
}

async fn send_updates_to_subscribers(bot: Bot, ctx: &BotContext) -> Result<()> {
    let latest_post = fetch_latest_post(ctx)?;

    let mut curr_latest_post = ctx.latest_post.lock().await;
    if curr_latest_post.as_ref() == Some(&latest_post) || curr_latest_post.is_none() {
        curr_latest_post.replace(latest_post);
        return Ok(());
    } else {
        curr_latest_post.replace(latest_post);
    }
    let curr_latest_post = curr_latest_post;

    let post_text = format_post(
        curr_latest_post
            .as_ref()
            .expect("Bug: Unwrap on latest post failed after setting it"),
    );
    for chat in Chat::list(&ctx.db_pool).await? {
        let chat_id = chat.get_chat_id();
        log::info!("Sending newest post to chat: {}", chat_id);
        if let Err(err) = bot
            .send_message(chat_id, &post_text)
            .parse_mode(ParseMode::HTML)
            .send()
            .await
        {
            log::error!("{}", err);
        }
    }

    Ok(())
}

fn fetch_latest_post(ctx: &BotContext) -> Result<Item> {
    let channel = Channel::from_url(&ctx.rss_url).context("Unable to fetch latest post")?;
    let item = channel
        .items()
        .get(0)
        .cloned()
        .context("No blog posts available")?;
    Ok(item)
}

async fn check_perm(cx: &UpdateWithCx<Message>, bot_ctx: &BotContext) -> Result<()> {
    let from_user = cx
        .update
        .from()
        .context("Received update from no user")?
        .username
        .as_ref()
        .context("User has no username")?;

    if bot_ctx.admins.contains(from_user).not() {
        cx.reply_to(
            "You don't have admin privileges. Write @robinhundt if you feel like \
        you deserve them.",
        )
        .send()
        .await?;
        Err(anyhow!("{} has no admin permissions.", from_user))
    } else {
        Ok(())
    }
}

fn format_post(post: &Item) -> String {
    let title = post.title().unwrap_or("No title!");
    let description = post.description().unwrap_or("No description!");
    let link = post.link().unwrap_or("No link!");
    format!("<b>{}</b>:\n{}\n{}", title, description, link)
}

impl FromStr for OptChatId {
    type Err = anyhow::Error;

    fn from_str(id: &str) -> Result<Self, Self::Err> {
        if id.is_empty() {
            return Ok(OptChatId(None));
        }
        let id: ChatId = if id.starts_with('@') {
            id.to_owned().into()
        } else {
            id.parse::<i64>()
                .context("ChatId must start with @ or be valid integer")?
                .into()
        };
        Ok(OptChatId(Some(id)))
    }
}
