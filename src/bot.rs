use std::sync::Arc;

use serenity::async_trait;
use serenity::model::gateway::Ready;
use serenity::model::prelude::command::{Command, CommandType};
use serenity::model::prelude::{Interaction, InteractionResponseType};
use serenity::model::voice::VoiceState;
use serenity::prelude::*;

use crate::db::DbManager;

struct Handler {
    db: Arc<DbManager>,
}

impl Handler {
    fn new(db: Arc<DbManager>) -> Self {
        Self { db }
    }
}

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, ctx: Context, ready: Ready) {
        Command::create_global_application_command(&ctx.http, |command| {
            command
                .name("opt_out")
                .kind(CommandType::ChatInput)
                .dm_permission(false)
                .description("Opt out of voice chat data aggregation")
        })
        .await
        .unwrap();
        Command::create_global_application_command(&ctx.http, |command| {
            command
                .name("opt_in")
                .kind(CommandType::ChatInput)
                .dm_permission(false)
                .description("Opt into voice chat data aggregation")
        })
        .await
        .unwrap();
        println!("{} is connected!", ready.user.name);
    }
    async fn voice_state_update(&self, _ctx: Context, new: VoiceState) {
        self.db
            .update_voicestate(new.user_id, new.channel_id, new.guild_id)
    }
    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        match interaction {
            Interaction::Ping(_) => {}
            Interaction::ApplicationCommand(command) => match command.data.name.as_str() {
                "opt_out" => {
                    self.db.add_excluded_user(command.user.id);
                    command
                        .create_interaction_response(&ctx.http, |response| {
                            response
                                .kind(InteractionResponseType::ChannelMessageWithSource)
                                .interaction_response_data(|data| {
                                    data.ephemeral(true).content(
                                        "You are now opting out of voice channel data aggregation.",
                                    )
                                })
                        })
                        .await
                        .unwrap();
                }
                "opt_in" => {
                    self.db.remove_excluded_user(command.user.id);
                    command
                        .create_interaction_response(&ctx.http, |response| {
                            response
                                .kind(InteractionResponseType::ChannelMessageWithSource)
                                .interaction_response_data(|data| {
                                    data.ephemeral(true).content(
                                        "You are now opting into voice channel data aggregation.",
                                    )
                                })
                        })
                        .await
                        .unwrap();
                }
                _ => {}
            },
            Interaction::MessageComponent(_) => todo!(),
            Interaction::Autocomplete(_) => todo!(),
            Interaction::ModalSubmit(_) => todo!(),
        }
    }
}

pub async fn build_bot(token: &str, db: Arc<DbManager>) -> serenity::Result<()> {
    // Set gateway intents, which decides what events the bot will be notified about
    let intents = GatewayIntents::GUILD_VOICE_STATES;

    let mut client = Client::builder(token, intents)
        .event_handler(Handler::new(db))
        .await?;

    client.start().await
}
