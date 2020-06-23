// use env_logger::Env;
// use ifi_blog_rs::BlogBot;
// use std::env;
use tokio;
use ifi_blog_rs::run;

#[tokio::main]
async fn main() {
    pretty_env_logger::formatted_builder()
        .write_style(pretty_env_logger::env_logger::WriteStyle::Auto)
        .filter(
            Some(&env!("CARGO_PKG_NAME").replace("-", "_")),
            log::LevelFilter::Info,
        )
        .filter(Some("teloxide"), log::LevelFilter::Error)
        .init();
    run("https://blog.stud.uni-goettingen.de/informatikstudiendekanat/feed/", "chat_ids.db").await.unwrap()
}
