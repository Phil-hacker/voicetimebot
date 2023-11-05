use std::{env, path::PathBuf, sync::Arc};

use control_server::create_control_server;
use db::DbManager;

mod bot;
mod control_server;
mod db;

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();
    // Configure the client with your Discord bot token in the environment.
    let token = env::var("DISCORD_TOKEN").expect("Expected a token in the environment");
    let db_path = env::var("DB_PATH").expect("Expected a DB_PATH in enviroment");
    let db: Arc<DbManager> = DbManager::open(PathBuf::from(&db_path))
        .unwrap_or_else(|_| DbManager::new())
        .into();
    let db1 = db.clone();
    create_control_server(9500, db1, &db_path);
    if let Err(why) = bot::build_bot(&token, db.clone()).await {
        println!("Client error: {:?}", why);
    }
}
