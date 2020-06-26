mod chat_ids;
use chat_ids::ChatIDs;

#[macro_use]
extern crate anyhow;

use teloxide::prelude::*;
use teloxide::types::{ParseMode, ChatId};
use teloxide::utils::command::BotCommand;
use futures::{select};
use futures_util::FutureExt;
use anyhow::{Result, Context};
use std::env;
use std::sync::Arc;
use tokio::time::{delay_for, Duration};
use tokio::sync::Mutex;
use rss::{Item, Channel};
use std::path::Path;
use std::ops::Not;
use std::collections::HashSet;
use tokio::fs::File;
use tokio::io::AsyncReadExt;
use std::iter::FromIterator;


struct BotContext {
    rss_url: String,
    db: ChatIDs,
    latest_post: Mutex<Option<Item>>,
    admins: HashSet<String>
}

#[derive(BotCommand)]
#[command(
    rename = "lowercase",
    description = "IFIBlogBot by @robinhundt. \
    Admins can start/stop updates for channels via /start @channelname ."
)]
enum Command {
    #[command(description = "Start the automatic blog updates")]
    Start,
    #[command(description = "Stop the automatic blog updates")]
    Stop,
    #[command(description = "Display an about message")]
    About,
    #[command(description = "Check whether you're currently subscribed")]
    Check,
    #[command(description = "Fetch the latest blog post")]
    Latest,
    #[command(description = "Display this help message")]
    Help
}

impl BotContext {
    async fn new(rss_url: &str, db_path: impl AsRef<Path>) -> Result<Self> {
        let db = ChatIDs::new(db_path)?;
        let mut buf = String::new();
        let mut admin_file = File::open("admin_list.txt").await?;
        admin_file.read_to_string(&mut buf).await?;
        let admins = HashSet::from_iter(buf.split_whitespace().map(|el| el.to_owned()));
        Ok(Self {
            rss_url: rss_url.into(),
            db,
            latest_post: Default::default(),
            admins
        })
    }
}

pub async fn run(rss_url: &str, db_path: impl AsRef<Path>) -> Result<()> {
    log::info!("Starting IfiBlogBot");
    let bot = Bot::from_env();
    let bot_ctx = Arc::new(BotContext::new(rss_url, db_path).await?);
    let bot_ctx2 = bot_ctx.clone();
    let dispatcher = Dispatcher::new(bot.clone()).messages_handler( |rx| handle_commands(rx, bot_ctx2));
    let dispatch = dispatcher.dispatch();
    let mut dispatch = Box::pin(dispatch).fuse();
    return select! {
         _ = dispatch => Err(anyhow!("Dispatcher stopped working")),
         _ = run_recurring_tasks(bot, bot_ctx).fuse() => Err(anyhow!("Recurring updates stopped working"))
    };
}

async fn handle_commands(rx: DispatcherHandlerRx<Message>, bot_ctx: Arc<BotContext>) {
    let bot_name = env::var("BOT_NAME").expect("Env var 'BOT_NAME' must be set");
    let bot_ctx = &bot_ctx;
    rx
        .commands::<Command, &str>(&bot_name)
        .for_each_concurrent(None, |(cx, command, args)| async move {
            action(cx, bot_ctx, command, &args).await.log_on_error().await;
        })
        .await;
}

async fn action(
    cx: DispatcherHandlerCx<Message>,
    bot_ctx: &BotContext,
    command: Command,
    args: &[String],
) -> Result<()> {
    match command {
        Command::Help => {
            cx.answer(Command::descriptions()).send().await.map(drop)?
        }
        Command::Start => start(&cx, bot_ctx, args).await?,
        Command::Stop => stop(&cx, bot_ctx, args).await?,
        Command::Check => check(&cx, bot_ctx, args).await?,
        Command::Latest => latest(&cx, bot_ctx).await?,
        Command::About => about(&cx).await?
    };

    Ok(())
}

async fn start(cx: &DispatcherHandlerCx<Message>, bot_ctx: &BotContext, args: &[String]) -> Result<()> {
    let (id, needs_admin) = match args {
        [id, ..] => (parse_id(id)?, true),
        _ => (cx.chat_id().into(), false)
    };

    if needs_admin {
        check_perm(cx, bot_ctx).await?;
    }

    bot_ctx.db.put(&id).context("Unable to store ID in DB")?;
    cx.bot.send_message(id.clone(),"You are now subscribed to the IfIBlog. The latest post is:")
        .send()
        .await?;
    let post = format_post(&fetch_latest_post(bot_ctx)?);
    cx.bot.send_message(id, post).parse_mode(ParseMode::HTML).send().await?;
    Ok(())
}

async fn stop(cx: &DispatcherHandlerCx<Message>, bot_ctx: &BotContext, args: &[String]) -> Result<()> {
    let (id, needs_admin) = match args {
        [id, ..] => (parse_id(id)?, true),
        _ => (cx.chat_id().into(), false)
    };

    if needs_admin {
        check_perm(cx, bot_ctx).await?;
    }
    bot_ctx.db.remove(&id)?;
    cx.bot
        .send_message(id, "You are now unsubscribed from the IfIBlog.").send()
        .await?;
    Ok(())
}

async fn check(cx: &DispatcherHandlerCx<Message>, bot_ctx: &BotContext, args: &[String]) -> Result<()> {
    let (id, needs_admin) = match args {
        [id, ..] => (parse_id(id)?, true),
        _ => (cx.chat_id().into(), false)
    };

    if needs_admin {
        check_perm(cx, bot_ctx).await?;
    }
    let pronoun = if needs_admin {format!("{} is", id)} else {"You're".to_owned()};
    let reply = if bot_ctx.db.contains(&id) {
        format!("{} currently subscribed to the blog. Enter /stop to unsubscribe.", pronoun)
    } else {
        format!("{} currently not subscribed to the blog. Enter /start to subscribe.", pronoun)
    };
    cx.reply_to(reply).send().await?;
    Ok(())
}

async fn latest(cx: &DispatcherHandlerCx<Message>, bot_ctx: &BotContext) -> Result<()> {
    let post = fetch_latest_post(bot_ctx)?;
    let post_text = format_post(&post);
    cx.reply_to(post_text).parse_mode(ParseMode::HTML).send().await?;
    Ok(())
}

async fn about(cx: &DispatcherHandlerCx<Message>) -> Result<()> {
    let reply = cx.reply_to(
        "Hi, I'm a small bot written by @robinhundt, that serves you the newest news \
        from the [CS deanery blog](https://blog.stud.uni-goettingen.de/informatikstudiendekanat/)\n\
        You can look at my source code on [gitlab](https://gitlab.gwdg.de/robinwilliam.hundt/ifi-blog-rs).\n\
        I'm written in Rust with bleeding edge features! :D");
    reply.parse_mode(ParseMode::MarkdownV2).send().await?;
    Ok(())
}

async fn run_recurring_tasks(bot: Arc<Bot>, ctx: Arc<BotContext>) {
    log::info!("Starting recurring tasks loop...");
    log::info!("Subscribed chats: {:?}", ctx.db.iter().collect::<Vec<_>>());
    loop {
        let ret = send_updates_to_subscribers(bot.clone(), &ctx).await;
        if let Err(err) = ret {
            log::error!("{}", err);
        }
        delay_for(Duration::from_secs(600)).await;
    }
}

async fn send_updates_to_subscribers(bot: Arc<Bot>, ctx: &BotContext) -> Result<()> {
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
    for chat_id in ctx.db.iter() {
        log::info!("Sending newest post to chat_id: {}", chat_id);
        if let Err(err) = bot.send_message(chat_id, &post_text).parse_mode(ParseMode::HTML).send()
            .await {
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

fn parse_id(id: &str) -> Result<ChatId> {
    let id = if id.starts_with("@") {
        id.to_owned().into()
    } else {
        id.parse::<i64>()?.into()
    };
    Ok(id)
}

async fn check_perm(cx: &DispatcherHandlerCx<Message>, bot_ctx: &BotContext) -> Result<()> {
    let from_user = cx.update.from().context("Received update from no user")?
        .username.as_ref().context("User has no username")?;

    if bot_ctx.admins.contains(from_user).not() {
        cx.reply_to("You don't have admin privileges. Write @robinhundt if you feel like \
        you deserve them.").send().await?;
        Err(anyhow!("{} has no admin permissions.", from_user).into())
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