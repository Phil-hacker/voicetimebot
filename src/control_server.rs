use std::{
    io::Read,
    net::{Ipv4Addr, SocketAddrV4, TcpListener},
    path::PathBuf,
    sync::{mpsc, Arc},
    thread,
    time::Instant,
};

use crate::{db::DbManager, SAVE_INTERVALL};

pub fn create_control_server(port: u16, db: Arc<DbManager>, db_path: &str) {
    let db_path = PathBuf::from(db_path);
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, port)).unwrap();
        loop {
            if let Ok(connection) = listener.accept() {
                let _ = sender.send(connection);
            }
        }
    });
    thread::spawn(move || {
        let mut last_save = Instant::now();
        let mut connections = Vec::new();
        let mut buffer = [0u8; 1 << 16];
        loop {
            if let Ok(connection) = receiver.try_recv() {
                if let Ok(_) = connection.0.set_nonblocking(true) {
                    connections.push(connection.0);
                }
            }
            for connection in connections.iter_mut() {
                if let Ok(size) = connection.read(&mut buffer) {
                    if size == 0 {
                        continue;
                    }
                    if let Ok(command) = std::str::from_utf8(&buffer[0..size]).map(|v| v.trim()) {
                        match command {
                            "save" => db.save_db(db_path.clone()),
                            "stop" => db.stop_and_save_db(db_path.clone()),
                            _ => {}
                        }
                    }
                }
            }
            if last_save.elapsed().as_secs() > SAVE_INTERVALL {
                last_save = Instant::now();
                db.save_db(db_path.clone());
            }
        }
    });
}
