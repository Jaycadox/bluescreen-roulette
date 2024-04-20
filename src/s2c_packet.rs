use serde::{Deserialize, Serialize};

use crate::server::Game;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum S2cPacket {
    SyncPlayerList(bool, Vec<String>),
    SyncGame(Game),
    KillYourselfNow,
    Disconnect(String),
}
