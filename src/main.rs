use std::{env, sync::Arc};

use db::Db;

mod bot;
mod db;

#[tokio::main]
async fn main() {
    // Configure the client with your Discord bot token in the environment.
    let token = env::var("DISCORD_TOKEN").expect("Expected a token in the environment");
    let db: Arc<Db> = Db::new().into();
    if let Err(why) = bot::build_bot(&token, db.clone()).await {
        println!("Client error: {:?}", why);
    }
}
