use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub enum C2sPacket {
    CreatePlayer(String),
    KeyPress(char),
    KeyRelease(char),
    HostStartGame,
}
