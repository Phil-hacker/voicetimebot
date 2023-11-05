use std::sync::Arc;

use serenity::async_trait;
use serenity::model::gateway::Ready;
use serenity::model::prelude::application_command::CommandDataOptionValue;
use serenity::model::prelude::command::{Command, CommandType};
use serenity::model::prelude::{ChannelType, Interaction, InteractionResponseType, UserId};
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
        Command::create_global_application_command(&ctx.http, |command| {
            command
                .name("get_vc_time")
                .kind(CommandType::ChatInput)
                .dm_permission(false)
                .description("Get VC time of a user")
                .create_option(|option| {
                    option
                        .name("user")
                        .description("User that should be queried")
                        .required(true)
                        .kind(serenity::model::prelude::command::CommandOptionType::User)
                })
                .create_option(|option| {
                    option
                        .name("channel")
                        .description("Channel that should be queried")
                        .kind(serenity::model::prelude::command::CommandOptionType::Channel)
                        .channel_types(&[ChannelType::Voice])
                        .required(false)
                })
        })
        .await
        .unwrap();
        Command::create_global_application_command(&ctx.http, |command| {
            command
                .name("leaderboard")
                .kind(CommandType::ChatInput)
                .dm_permission(false)
                .description("Load the Leaderboard")
                .create_option(|option| {
                    option
                        .name("channel")
                        .description("Channel that the leaderboard should be for")
                        .kind(serenity::model::prelude::command::CommandOptionType::Channel)
                        .channel_types(&[ChannelType::Voice])
                        .required(false)
                })
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
                "get_vc_time" => {
                    let args = &command.data.options;
                    let channel = args.iter().find(|v| v.name == "channel").and_then(|v| {
                        if let CommandDataOptionValue::Channel(channel) =
                            v.resolved.as_ref().unwrap()
                        {
                            Some(channel.id)
                        } else {
                            None
                        }
                    });
                    let user = args
                        .iter()
                        .find(|v| v.name == "user")
                        .and_then(|v| {
                            if let CommandDataOptionValue::User(user, _) =
                                v.resolved.as_ref().unwrap()
                            {
                                Some(user.id)
                            } else {
                                None
                            }
                        })
                        .unwrap();
                    self.db.get_time(
                        UserId(user.0),
                        command.guild_id.unwrap(),
                        channel,
                        ctx.http,
                        command,
                    );
                }
                "leaderboard" => {
                    let args = &command.data.options;
                    let channel = args.iter().find(|v| v.name == "channel").and_then(|v| {
                        if let CommandDataOptionValue::Channel(channel) =
                            v.resolved.as_ref().unwrap()
                        {
                            Some(channel.id)
                        } else {
                            None
                        }
                    });
                    self.db
                        .get_leaderboard(command.guild_id.unwrap(), channel, ctx.http, command);
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
