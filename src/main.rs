use env_logger::Env;
use ifi_blog_rs::BlogBot;
use std::env;

#[tokio::main]
async fn main() -> Result<(), ifi_blog_rs::BotError> {
    env_logger::from_env(Env::default().default_filter_or("info")).init();
    let token = env::var("TELEGRAM_BOT_TOKEN").expect("TELEGRAM_BOT_TOKEN not set");
    let mut bot = BlogBot::new(
        token,
        "https://blog.stud.uni-goettingen.de/informatikstudiendekanat/feed/",
        "chat_ids.db",
    )
    .unwrap();

    bot.run().await
}
