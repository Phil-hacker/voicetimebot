use std::{env, sync::Arc, thread};

use db::DbManager;

mod bot;
mod db;

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();
    // Configure the client with your Discord bot token in the environment.
    let token = env::var("DISCORD_TOKEN").expect("Expected a token in the environment");
    let db: Arc<DbManager> = DbManager::new().into();
    let db1 = db.clone();
    thread::spawn(move || loop {
        std::io::stdin().read_line(&mut String::new()).unwrap();
        db1.save_db("test.db".into());
    });
    if let Err(why) = bot::build_bot(&token, db.clone()).await {
        println!("Client error: {:?}", why);
    }
}
