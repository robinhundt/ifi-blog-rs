use refinery::config::{Config, ConfigDbType};
use std::env;
use std::fs::OpenOptions;
// This build.rs is needed to apply migrations *before* the lib containing
// the sqlx queries is compiled. This is necessary when compiling the
// crate with a fresh database.

mod embedded {
    use refinery::embed_migrations;

    embed_migrations!("migrations");
}

pub fn run_migrations() {
    let db_url = env::var("DATABASE_URL").expect("`DATABASE_URL` must be set");
    let db_url = db_url
        .strip_prefix("sqlite:")
        .expect("`DATABASE_URL` must have `sqlite:` prefix");
    {
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(&db_url)
            .expect("Unable to create or open db file");
    }
    let mut config = Config::new(ConfigDbType::Sqlite).set_db_path(&db_url);
    eprintln!("Applying migrations");
    embedded::migrations::runner()
        .run(&mut config)
        .expect("Unable to run migrations");
}

fn main() {
    run_migrations()
}
