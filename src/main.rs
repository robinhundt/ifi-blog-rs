use ifi_blog_rs::BlogBot;
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let token = env::var("TELEGRAM_BOT_TOKEN").expect("TELEGRAM_BOT_TOKEN not set");
    let mut bot = BlogBot::new(
        token,
        "https://blog.stud.uni-goettingen.de/informatikstudiendekanat/feed/",
        "chat_ids.db",
    )
    .unwrap();

    bot.run().await
}
