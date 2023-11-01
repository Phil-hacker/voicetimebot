use std::{collections::HashSet, env, sync::Arc};

use db::DbManager;

mod bot;
mod db;

#[tokio::main]
async fn main() {
    // Configure the client with your Discord bot token in the environment.
    let token = env::var("DISCORD_TOKEN").expect("Expected a token in the environment");
    let db: Arc<DbManager> = DbManager::new(HashSet::default()).into();
    if let Err(why) = bot::build_bot(&token, db.clone()).await {
        println!("Client error: {:?}", why);
    }
}
