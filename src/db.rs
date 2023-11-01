use std::{
    collections::HashSet,
    sync::{mpsc::Sender, Arc, Mutex},
    thread::{self, JoinHandle},
};

use serenity::model::prelude::UserId;

pub struct Db {
    excluded_users: Arc<Mutex<HashSet<UserId>>>,
    excluded_users_channel: Sender<DbMessage>,
    excluded_users_thread: JoinHandle<()>,
}

impl Db {
    pub fn new() -> Self {
        let excluded_users: Arc<Mutex<HashSet<UserId>>> = Arc::default();
        let excluded_users_cloned = excluded_users.clone();
        let (excluded_users_channel, read_channel) = std::sync::mpsc::channel();
        let excluded_users_thread = thread::spawn(move || {
            let mut lock = excluded_users_cloned.lock().unwrap();
            while let Ok(message) = read_channel.recv() {
                match message {
                    DbMessage::AddUserToOptOut { user_id } => {
                        lock.insert(user_id);
                    }
                    DbMessage::IsUserOptOut { user_id, callback } => {
                        callback(lock.contains(&user_id));
                    }
                    DbMessage::RemoverUserToOptOut { user_id } => {
                        lock.remove(&user_id);
                    }
                }
            }
        });
        Self {
            excluded_users,
            excluded_users_thread,
            excluded_users_channel,
        }
    }
    pub fn add_excluded_user(&self, user_id: UserId) {
        self.excluded_users_channel
            .send(DbMessage::AddUserToOptOut { user_id })
            .unwrap();
    }
    pub fn remove_excluded_user(&self, user_id: UserId) {
        self.excluded_users_channel
            .send(DbMessage::RemoverUserToOptOut { user_id })
            .unwrap();
    }
    pub fn is_excluded_user(&self, user_id: UserId, callback: fn(bool) -> ()) {
        self.excluded_users_channel
            .send(DbMessage::IsUserOptOut { user_id, callback })
            .unwrap();
    }
}

pub enum DbMessage {
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
}
