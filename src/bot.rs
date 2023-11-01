use std::sync::Arc;

use serenity::async_trait;
use serenity::model::gateway::Ready;
use serenity::prelude::*;

use crate::db::Db;

struct Handler {
    db: Arc<Db>,
}

impl Handler {
    fn new(db: Arc<Db>) -> Self {
        Self { db }
    }
}

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, _: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);
    }
}

pub async fn build_bot(token: &str, db: Arc<Db>) -> serenity::Result<()> {
    // Set gateway intents, which decides what events the bot will be notified about
    let intents = GatewayIntents::GUILD_VOICE_STATES;

    let mut client = Client::builder(token, intents)
        .event_handler(Handler::new(db))
        .await?;

    client.start().await
}
