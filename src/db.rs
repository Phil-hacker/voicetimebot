use std::{
    collections::{HashMap, HashSet},
    sync::{mpsc::Sender, Arc, Mutex},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use serenity::model::prelude::{ChannelId, GuildId, UserId};

#[derive(Debug, Default, Hash, PartialEq, PartialOrd, Ord, Eq, Clone, Copy)]
pub struct Seconds(u64);

pub struct VoiceState {
    time: Instant,
    channel: ChannelId,
    guild: GuildId,
}

pub struct Db {
    excluded_users: HashSet<UserId>,
    voice_times: HashMap<UserId, HashMap<(GuildId, ChannelId), Seconds>>,
    voice_states: HashMap<UserId, VoiceState>,
}

impl Db {
    fn new(excluded_users: HashSet<UserId>) -> Self {
        Self {
            excluded_users,
            voice_times: HashMap::default(),
            voice_states: HashMap::default(),
        }
    }
    fn add_time_to_user(
        &mut self,
        user_id: UserId,
        guild_id: GuildId,
        channel_id: ChannelId,
        duration: Duration,
    ) {
        if !self.voice_times.contains_key(&user_id) {
            self.voice_times.insert(user_id, HashMap::default());
        }
        let user_time_map = self.voice_times.get_mut(&user_id).unwrap();
        let mut user_time = user_time_map
            .get(&(guild_id, channel_id))
            .map(|v| *v)
            .unwrap_or_default();
        user_time.0 += duration.as_secs();
        println!("{}|{}|{:?}", user_id, channel_id, user_time);
        user_time_map.insert((guild_id, channel_id), user_time);
    }
    fn handle_voicestate(&mut self, user_id: UserId, voicestate: Option<VoiceState>) {
        let voicestate = if let Some(voicestate) = voicestate {
            self.voice_states.insert(user_id, voicestate)
        } else {
            self.voice_states.remove(&user_id)
        };
        if let Some(voicestate) = voicestate {
            self.add_time_to_user(
                user_id,
                voicestate.guild,
                voicestate.channel,
                voicestate.time.elapsed(),
            );
        }
    }
    fn handle_message(&mut self, message: DbMessage) {
        match message {
            DbMessage::AddUserToOptOut { user_id } => {
                self.excluded_users.insert(user_id);
            }
            DbMessage::IsUserOptOut { user_id, callback } => {
                callback(self.excluded_users.contains(&user_id));
            }
            DbMessage::RemoverUserToOptOut { user_id } => {
                self.excluded_users.remove(&user_id);
            }
            DbMessage::UpdateVoicestate {
                user_id,
                channel_id,
                guild_id,
                time,
            } => {
                let mut voicestate = None;
                if let Some(channel_id) = channel_id {
                    if let Some(guild_id) = guild_id {
                        voicestate = Some(VoiceState {
                            time,
                            channel: channel_id,
                            guild: guild_id,
                        });
                    }
                };
                self.handle_voicestate(user_id, voicestate);
            }
        }
    }
}

pub struct DbManager {
    _db: Arc<Mutex<Db>>,
    db_channel: Sender<DbMessage>,
    _db_thread: JoinHandle<()>,
}

impl DbManager {
    pub fn new(excluded_users: HashSet<UserId>) -> Self {
        let db: Arc<Mutex<Db>> = Arc::new(Mutex::new(Db::new(excluded_users)));
        let db_cloned = db.clone();
        let (db_channel, read_channel) = std::sync::mpsc::channel();
        let _db_thread = thread::spawn(move || {
            let mut db = db_cloned.lock().unwrap();
            while let Ok(message) = read_channel.recv() {
                db.handle_message(message)
            }
        });
        Self {
            _db: db,
            _db_thread,
            db_channel,
        }
    }
    pub fn add_excluded_user(&self, user_id: UserId) {
        self.db_channel
            .send(DbMessage::AddUserToOptOut { user_id })
            .unwrap();
    }
    pub fn remove_excluded_user(&self, user_id: UserId) {
        self.db_channel
            .send(DbMessage::RemoverUserToOptOut { user_id })
            .unwrap();
    }
    pub fn is_excluded_user(&self, user_id: UserId, callback: fn(bool) -> ()) {
        self.db_channel
            .send(DbMessage::IsUserOptOut { user_id, callback })
            .unwrap();
    }
    pub fn update_voicestate(
        &self,
        user_id: UserId,
        channel_id: Option<ChannelId>,
        guild_id: Option<GuildId>,
    ) {
        self.db_channel
            .send(DbMessage::UpdateVoicestate {
                user_id,
                channel_id,
                guild_id,
                time: Instant::now(),
            })
            .unwrap();
    }
}

enum DbMessage {
    AddUserToOptOut {
        user_id: UserId,
    },
    IsUserOptOut {
        user_id: UserId,
        callback: fn(bool) -> (),
    },
    RemoverUserToOptOut {
        user_id: UserId,
    },
    UpdateVoicestate {
        user_id: UserId,
        channel_id: Option<ChannelId>,
        guild_id: Option<GuildId>,
        time: Instant,
    },
}
