use serenity::{
    http::Http,
    model::prelude::{
        application_command::ApplicationCommandInteraction, ChannelId, GuildId,
        InteractionApplicationCommandCallbackDataFlags, UserId,
    },
    utils::MessageBuilder,
};
use std::{
    collections::{HashMap, HashSet},
    fs::File,
    io::{Read, Write},
    path::PathBuf,
    sync::{mpsc::Sender, Arc, Mutex},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use tokio::runtime::Runtime;

#[derive(Debug, Default, Hash, PartialEq, PartialOrd, Ord, Eq, Clone, Copy)]
pub struct Seconds(u64);

pub struct VoiceState {
    time: Instant,
    channel: ChannelId,
    guild: GuildId,
}

const SILENT_FLAG: InteractionApplicationCommandCallbackDataFlags =
    unsafe { InteractionApplicationCommandCallbackDataFlags::from_bits_unchecked(1 << 12) };

pub struct Db {
    excluded_users: HashSet<UserId>,
    voice_times: HashMap<UserId, HashMap<(GuildId, ChannelId), Seconds>>,
    voice_states: HashMap<UserId, VoiceState>,
}

impl Db {
    /// Creates a new empty [Db]
    fn new() -> Self {
        Self {
            excluded_users: HashSet::default(),
            voice_times: HashMap::default(),
            voice_states: HashMap::default(),
        }
    }
    /// Writes the [Db] to a [Writer][Write]
    /// Returns an [error][std::io::Error] if writing failed
    fn to_bytes(&self, writer: &mut dyn Write) -> Result<(), std::io::Error> {
        writer.write_all(&(self.excluded_users.len() as u64).to_le_bytes())?;
        for user in self.excluded_users.iter() {
            writer.write_all(&user.0.to_le_bytes())?;
        }
        writer.write_all(&(self.voice_times.len() as u64).to_le_bytes())?;
        for (user, times) in self.voice_times.iter() {
            writer.write_all(&user.0.to_le_bytes())?;
            writer.write_all(&times.len().to_le_bytes())?;
            for ((guild, channel), time) in times.iter() {
                writer.write_all(&guild.0.to_le_bytes())?;
                writer.write_all(&channel.0.to_le_bytes())?;
                writer.write_all(&time.0.to_le_bytes())?;
            }
        }
        writer.flush()
    }
    /// Reads the [Db] from a [Reader][Read]
    /// Returns an [error][std::io::Error] if writing failed
    fn from_bytes(reader: &mut dyn Read) -> Result<Db, std::io::Error> {
        let mut db = Self::new();
        let len = read_u64(reader)?;
        for _ in 0..len {
            let user = UserId(read_u64(reader)?);
            db.excluded_users.insert(user);
        }
        let len = read_u64(reader)?;
        for _ in 0..len {
            let user_id = UserId(read_u64(reader)?);
            let len = read_u64(reader)?;
            let mut user_times = HashMap::default();
            for _ in 0..len {
                user_times.insert(
                    (GuildId(read_u64(reader)?), ChannelId(read_u64(reader)?)),
                    Seconds(read_u64(reader)?),
                );
            }
            db.voice_times.insert(user_id, user_times);
        }
        Ok(db)
    }
    fn get_time(&self, user: UserId, guild: GuildId, channel_id: Option<ChannelId>) -> Seconds {
        match self.voice_times.get(&user) {
            Some(data) => Seconds(
                data.iter()
                    .filter(|v| v.0 .0 == guild && channel_id.map(|c| c == v.0 .1).unwrap_or(true))
                    .map(|v| v.1 .0)
                    .sum(),
            ),
            None => Seconds(0),
        }
    }
    fn add_time_to_user(
        &mut self,
        user_id: UserId,
        guild_id: GuildId,
        channel_id: ChannelId,
        duration: Duration,
    ) {
        self.voice_times
            .entry(user_id)
            .or_insert_with(HashMap::default);
        let user_time_map = self.voice_times.get_mut(&user_id).unwrap();
        let mut user_time = user_time_map
            .get(&(guild_id, channel_id))
            .copied()
            .unwrap_or_default();
        user_time.0 += duration.as_secs();
        println!("{}|{}|{:?}", user_id, channel_id, user_time);
        user_time_map.insert((guild_id, channel_id), user_time);
    }
    fn is_excluded_user(&self, user_id: &UserId) -> bool {
        self.excluded_users.contains(user_id)
    }
    fn handle_voicestate(&mut self, user_id: UserId, voicestate: Option<VoiceState>) {
        if self.is_excluded_user(&user_id) {
            return;
        }
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
    fn handle_message(&mut self, message: DbMessage, tokio: &mut Runtime) {
        match message {
            DbMessage::AddUserToOptOut { user_id } => {
                self.excluded_users.insert(user_id);
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
            DbMessage::SaveDb { path } => {
                let mut file = File::create(path).unwrap();
                self.to_bytes(&mut file).unwrap();
                println!("Saved DB");
            }
            DbMessage::GetTime {
                user_id,
                guild_id,
                channel_id,
                http,
                command,
            } => {
                tokio.spawn(send_time_message(
                    user_id,
                    guild_id,
                    channel_id,
                    http,
                    command,
                    self.get_time(user_id, guild_id, channel_id),
                ));
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
    fn from_db(db: Db) -> Self {
        let db: Arc<Mutex<Db>> = Arc::new(Mutex::new(db));
        let db_cloned = db.clone();
        let (db_channel, read_channel) = std::sync::mpsc::channel();
        let _db_thread = thread::spawn(move || {
            let mut tokio = tokio::runtime::Runtime::new().unwrap();
            let mut db = db_cloned.lock().unwrap();
            while let Ok(message) = read_channel.recv() {
                db.handle_message(message, &mut tokio)
            }
        });
        Self {
            _db: db,
            _db_thread,
            db_channel,
        }
    }
    pub fn open(path: PathBuf) -> Result<Self, std::io::Error> {
        let mut file = File::open(path)?;
        Ok(Self::from_db(Db::from_bytes(&mut file)?))
    }
    pub fn new() -> Self {
        Self::from_db(Db::new())
    }
    pub fn save_db(&self, path: PathBuf) {
        self.db_channel.send(DbMessage::SaveDb { path }).unwrap();
    }
    pub fn get_time(
        &self,
        user_id: UserId,
        guild_id: GuildId,
        channel_id: Option<ChannelId>,
        http: Arc<Http>,
        command: ApplicationCommandInteraction,
    ) {
        self.db_channel
            .send(DbMessage::GetTime {
                user_id,
                guild_id,
                channel_id,
                http,
                command,
            })
            .unwrap()
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
    RemoverUserToOptOut {
        user_id: UserId,
    },
    UpdateVoicestate {
        user_id: UserId,
        channel_id: Option<ChannelId>,
        guild_id: Option<GuildId>,
        time: Instant,
    },
    SaveDb {
        path: PathBuf,
    },
    GetTime {
        user_id: UserId,
        guild_id: GuildId,
        channel_id: Option<ChannelId>,
        http: Arc<Http>,
        command: ApplicationCommandInteraction,
    },
}

fn read_u64(reader: &mut dyn Read) -> Result<u64, std::io::Error> {
    let mut buffer = [0u8; 8];
    reader.read_exact(&mut buffer)?;
    Ok(u64::from_le_bytes(buffer))
}

async fn send_time_message(
    user_id: UserId,
    _guild_id: GuildId,
    channel_id: Option<ChannelId>,
    http: Arc<Http>,
    command: ApplicationCommandInteraction,
    time: Seconds,
) {
    let time = humantime::format_duration(Duration::from_secs(time.0)).to_string();
    let mut msg = MessageBuilder::new();
    msg.push(format!("<@{}>", user_id.0))
        .push(" war ")
        .push(time);
    let text = if let Some(channel) = channel_id {
        msg.push(" in ").channel(channel).build()
    } else {
        msg.push(" in einem VC").build()
    };
    command
        .create_interaction_response(&http, |interaction| {
            interaction.interaction_response_data(|data| data.content(text).flags(SILENT_FLAG))
        })
        .await
        .unwrap();
}
        })
        .await
        .unwrap();
}
