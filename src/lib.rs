mod chat_ids;
use chat_ids::ChatIDs;

use std::time::Duration;

use futures::future::join;
use futures::StreamExt;
use std::error::Error;
use std::path::Path;
use telegram_bot::prelude::*;
use telegram_bot::{Api, GetMe, Message, MessageKind, ParseMode, UpdateKind, User};
use tokio::timer::delay_for;

use rss::{Channel, Item};

pub type BoxError = Box<dyn Error>;

pub type BoxResult<T> = Result<T, BoxError>;

pub struct BlogBot {
    rss_url: String,
    api: Api,
    me: Option<User>,
    db: ChatIDs,
    latest_post: Option<Item>,
}

macro_rules! routes {
    ( $self:ident, $data:expr, $msg:expr, $route:ident, $($routes:ident),* ) => {
        {
                let data = $data.trim_end();
                let bot_username = $self
                    .me
                    .as_ref()
                    .expect("Me not initialized!")
                    .username
                    .as_ref()
                    .expect("Bots should have a username");

                if data == format!("/{}", stringify!($route)) ||
                    data == format!("/{}@{}", stringify!($route), &bot_username) {
                    $self.$route($msg).await?
                }
                $(
                    else if data == format!("/{}", stringify!($routes)) ||
                        data == format!("/{}@{}", stringify!($routes), &bot_username) {
                        $self.$routes($msg).await?
                    }
                )*
        }
    };
}

impl BlogBot {
    pub fn new<B, R, P>(bot_token: B, rss_url: R, db_path: P) -> BoxResult<Self>
    where
        B: AsRef<str>,
        R: Into<String>,
        P: AsRef<Path>,
    {
        Ok(Self {
            rss_url: rss_url.into(),
            api: Api::new(bot_token),
            me: None,
            db: ChatIDs::new(db_path)?,
            latest_post: None,
        })
    }

    pub async fn run(&mut self) -> BoxResult<()> {
        let me = self.api.send(GetMe).await?;
        self.me.replace(me);
        join(self.run_process_updates(), self.run_recurring_tasks()).await;
        Ok(())
    }


    async fn run_recurring_tasks(&self) {
        loop {
            let ret = self.send_updates_to_subscribers().await;
            delay_for(Duration::from_secs(600)).await;
            if let Err(err) = ret {
                dbg!(err);
            }
        }
    }

    async fn run_process_updates(&self) {
        let mut stream = self.api.stream();
        while let Some(update) = stream.next().await {
            let update = match update {
                Ok(update) => update,
                Err(err) => {
                    dbg!(format!("ERROR: {:?}", err));
                    continue;
                }
            };
            if let UpdateKind::Message(msg) = update.kind {
                let ret = self.process(&msg).await;
                if let Err(err) = ret {
                    let ret =
                        self.api
                            .send(msg.text_reply(format!(
                                "An error ocured during your request:\n{}",
                                err
                            )))
                            .await;
                    if let Err(err) = ret {
                        dbg!(format!("ERROR: {:?}", err));
                    }
                }
            }
        }
        ()
    }

    async fn process(&self, msg: &Message) -> BoxResult<()> {
        match msg.kind {
            MessageKind::Text { ref data, .. } => {
                routes!(self, data, msg, start, stop, check, latest, about);
            }
            _ => (),
        };
        Ok(())
    }

    async fn start(&self, msg: &Message) -> BoxResult<()> {
        let id = msg.chat.id();
        self.db.put(id)?;
        self.api
            .send(msg.text_reply("You are now subscribed to the IfIBlog."))
            .await?;
        Ok(())
    }

    async fn stop(&self, msg: &Message) -> BoxResult<()> {
        let id = msg.chat.id();
        self.db.remove(id)?;
        self.api
            .send(msg.text_reply("You are now unsubscribed from the IfIBlog."))
            .await?;
        Ok(())
    }

    async fn check(&self, msg: &Message) -> BoxResult<()> {
        let id = msg.chat.id();
        let reply = if self.db.contains(id) {
            "You're currently subscribed to the blog. Enter /stop to unsubscribe."
        } else {
            "You're currently not subscribed to the blog. Enter /start to subscribe."
        };
        self.api.send(msg.text_reply(reply)).await?;
        Ok(())
    }

    async fn latest(&self, msg: &Message) -> BoxResult<()> {
        let post = self.fetch_latest_post()?;
        let post_text = format_post(&post);
        let mut reply = msg.text_reply(post_text);
        self.api.send(reply.parse_mode(ParseMode::Html)).await?;
        Ok(())
    }

    async fn about(&self, msg: &Message) -> BoxResult<()> {
        let mut reply = msg.text_reply(
            "Hi, im a small bot written by @robinhundt, that serves you the newest news \
            from the [CS deanery blog](https://blog.stud.uni-goettingen.de/informatikstudiendekanat/)\n\
            You can look at my source code on [gitlab](https://gitlab.gwdg.de/robinwilliam.hundt/ifi-blog-rs).\n\
            I'm written in Rust with bleeding edge features! :D");
        self.api.send(reply.parse_mode(ParseMode::Markdown)).await?;
        Ok(())
    }

    async fn send_updates_to_subscribers(&self) -> BoxResult<()> {
        let latest_post = self.fetch_latest_post()?;
        if self.latest_post.as_ref() == Some(&latest_post) {
            return Ok(());
        }
        let post_text = format_post(&latest_post);
        for chat_id in self.db.iter() {
            self.api
                .send(chat_id.text(&post_text).parse_mode(ParseMode::Html))
                .await?;
        }

        Ok(())
    }

    fn fetch_latest_post(&self) -> BoxResult<Item> {
        let channel = Channel::from_url(&self.rss_url)?;
        let item = channel
            .items()
            .get(0)
            .cloned()
            .ok_or("No blogposts available!")?;
        Ok(item)
    }
}

fn format_post(post: &Item) -> String {
    let title = post.title().unwrap_or("No title!");
    let description = post.description().unwrap_or("No description!");
    let link = post.link().unwrap_or("No link!");
    format!("<b>{}</b>:\n{}\n{}", title, description, link)
}
