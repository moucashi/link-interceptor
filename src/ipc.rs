use serde::{Deserialize, Serialize};
use std::{
    io::{Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    sync::mpsc::{self, Receiver},
    thread,
    time::Duration,
};

const IPC_ADDR: &str = "127.0.0.1:38761";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum IpcCommand {
    ShowMain,
    OpenIntercept { url: String },
}

pub fn try_send(command: &IpcCommand) -> bool {
    let Ok(addr) = IPC_ADDR.parse::<SocketAddr>() else {
        return false;
    };
    let Ok(mut stream) = TcpStream::connect_timeout(&addr, Duration::from_millis(150)) else {
        return false;
    };
    let Ok(payload) = serde_yaml::to_string(command) else {
        return false;
    };
    stream
        .set_write_timeout(Some(Duration::from_millis(300)))
        .is_ok()
        && stream.write_all(payload.as_bytes()).is_ok()
}

pub fn start_listener() -> Option<Receiver<IpcCommand>> {
    let listener = TcpListener::bind(IPC_ADDR).ok()?;
    let (sender, receiver) = mpsc::channel();

    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let sender = sender.clone();
            thread::spawn(move || {
                if let Some(command) = read_command(stream) {
                    let _ = sender.send(command);
                }
            });
        }
    });

    Some(receiver)
}

fn read_command(mut stream: TcpStream) -> Option<IpcCommand> {
    let mut payload = String::new();
    stream
        .set_read_timeout(Some(Duration::from_millis(500)))
        .ok()?;
    stream.read_to_string(&mut payload).ok()?;
    serde_yaml::from_str(&payload).ok()
}
