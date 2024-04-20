use std::{collections::HashSet, net::SocketAddr, time::Instant};

use egui_macroquad::egui::epaint::ahash::{HashMap, HashMapExt};
use rand::{seq::SliceRandom, Rng};
use serde::{Deserialize, Serialize};
use tokio::{
    net::TcpStream,
    sync::mpsc::{self, Receiver, Sender},
};
use tokio_util::sync::CancellationToken;

use crate::{c2s_packet::C2sPacket, packet_channel, s2c_packet::S2cPacket};

pub const PORT: u16 = 6666;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Game {
    pub queue: Vec<String>,
    pub progress: HashMap<char, u8>,
    pub fired: HashSet<char>,
    #[serde(skip)]
    last_frame: Option<Instant>,
    #[serde(skip)] // you wish
    trigger_key: char,
}

#[derive(Debug)]
pub struct Server {
    players: Vec<PacketPlayer>,
    game: Option<Game>,
}

#[derive(Debug)]
enum C2sMessage {
    PlayerConnect(String),
    Packet(C2sPacket),
    PlayerDisconnect,
}

#[derive(Debug)]
enum S2cMessage {
    Packet(S2cPacket),
    Disconnect(Option<String>),
}

#[derive(Debug)]
struct PacketPlayer {
    sock_addr: SocketAddr,
    sender: Sender<S2cMessage>,
    reciever: Receiver<C2sMessage>,
    name: String,
    host: bool,
}

impl PacketPlayer {
    async fn send_packet(&mut self, pack: S2cPacket) {
        let _ = self.sender.send(S2cMessage::Packet(pack)).await;
    }

    async fn disconnect(&mut self, reason: String) {
        let _ = self.sender.send(S2cMessage::Disconnect(Some(reason))).await;
    }
}

impl Server {
    async fn handle_client(
        stream: TcpStream,
        mut in_rx: Receiver<S2cMessage>,
        out_tx: Sender<C2sMessage>,
    ) {
        let (mut rx, tx) = packet_channel::async_channel::<S2cPacket, C2sPacket>(stream);
        let Some(Ok(C2sPacket::CreatePlayer(name))) = rx.recv().await else {
            return;
        };

        println!("got name: {name}");
        out_tx
            .send(C2sMessage::PlayerConnect(name.clone()))
            .await
            .unwrap();
        loop {
            match rx.try_recv() {
                Ok(Ok(packet)) => {
                    let Ok(_) = out_tx.send(C2sMessage::Packet(packet)).await else {
                        break;
                    };
                }
                Ok(Err(e)) => {
                    eprintln!("Err: {e}");
                    break;
                }
                Err(mpsc::error::TryRecvError::Disconnected) => {
                    break;
                }
                Err(mpsc::error::TryRecvError::Empty) => {}
            };
            match in_rx.try_recv() {
                Ok(packet) => match packet {
                    S2cMessage::Packet(packet) => {
                        let Ok(_) = tx.send(packet).await else {
                            break;
                        };
                    }
                    S2cMessage::Disconnect(msg) => {
                        let _ = tx
                            .send(S2cPacket::Disconnect(msg.unwrap_or(
                                "You have been disconnected from the server".to_string(),
                            )))
                            .await;
                        break;
                    }
                },
                Err(mpsc::error::TryRecvError::Disconnected) => {
                    let _ = tx
                        .send(S2cPacket::Disconnect(
                            "You have been disconnected from the server".to_string(),
                        ))
                        .await;
                    break;
                }
                Err(mpsc::error::TryRecvError::Empty) => {}
            }
            tokio::task::yield_now().await;
        }

        let _ = out_tx.send(C2sMessage::PlayerDisconnect).await;
        println!("closing connection");
    }

    pub async fn start() {
        let tcp = tokio::net::TcpListener::bind(format!("0.0.0.0:{PORT}"))
            .await
            .unwrap();
        println!("Server started");
        let mut s = Self {
            players: vec![],
            game: None,
        };
        let (pp_tx, pp_rx) = std::sync::mpsc::channel();
        let token = CancellationToken::new();
        let closed = token.clone();
        tokio::spawn(async move {
            loop {
                let (stream, sock) = tokio::select! {
                    res = tcp.accept() => {
                        let res = res.unwrap();
                        (res.0, res.1)
                    },
                    _ = closed.cancelled() => {
                        break;
                    }
                };
                let (tx, rx) = mpsc::channel(1024);
                let (reply_tx, reply_rx) = mpsc::channel(1024);
                let pp = PacketPlayer {
                    sock_addr: sock,
                    sender: tx,
                    reciever: reply_rx,
                    name: "???".to_string(),
                    host: false,
                };
                pp_tx.send(pp).unwrap();
                tokio::spawn(async move {
                    Self::handle_client(stream, rx, reply_tx).await;
                });
            }
            println!("Server stopped");
        });
        let mut had_players = false;
        loop {
            if let Ok(new_player) = pp_rx.try_recv() {
                let sock = new_player.sock_addr;
                s.players.push(new_player);
                s.player_joined(sock).await;
            }

            let mut pack_queue = vec![];
            let mut remove_queue = vec![];
            let mut should_resync_playerlist = false;

            for pl in &mut s.players {
                if let Ok(pack) = pl.reciever.try_recv() {
                    println!("got pack: {pack:?}");
                    let sock = pl.sock_addr;
                    match pack {
                        C2sMessage::Packet(packet) => {
                            pack_queue.push((sock, packet));
                        }
                        C2sMessage::PlayerConnect(name) => {
                            let host = !had_players;
                            had_players = true;
                            let name = name.trim();
                            if name.is_empty() {
                                remove_queue.push((
                                    sock,
                                    Some("Your username cannot be empty".to_string()),
                                ));
                                println!("Kicking for bad name");
                                continue;
                            }
                            pl.name = name.to_string();
                            pl.host = host;
                            should_resync_playerlist = true;
                        }
                        C2sMessage::PlayerDisconnect => {
                            if pl.host {
                                token.cancel();
                            }
                            remove_queue.push((sock, None));
                        }
                    }
                }
            }

            for (sock, reason) in remove_queue {
                s.remove_player(sock, reason).await;
            }

            if should_resync_playerlist {
                s.sync_playerlist().await;
            }

            for (sock, packet) in pack_queue {
                s.on_packet(sock, packet).await;
            }

            let new_playercount = s.players.len();

            tokio::task::yield_now().await;

            if new_playercount == 0 && had_players {
                token.cancel();
                break;
            }
            if let Some(game) = s.game.as_mut() {
                if let Some(last_frame) = game.last_frame {
                    if Instant::now().duration_since(last_frame).as_millis() >= 50 {
                        game.last_frame = Some(Instant::now());
                        s.tick().await;
                    }
                }
            }
        }
    }
}

impl Game {
    fn new(mut players: Vec<String>) -> Self {
        players.shuffle(&mut rand::thread_rng());
        Self {
            progress: HashMap::new(),
            queue: players,
            last_frame: Some(Instant::now()),
            fired: HashSet::new(),
            trigger_key: rand::thread_rng().gen_range(b'A'..=b'Z') as char,
        }
    }

    fn current(&self) -> String {
        self.queue[0].clone()
    }

    fn advance(&mut self) -> String {
        let first = self.queue.remove(0);
        self.queue.push(first.clone());
        first
    }
}

impl Server {
    async fn sync_playerlist(&mut self) {
        let playerlist = self
            .players
            .iter()
            .map(|p| p.name.to_string())
            .collect::<Vec<_>>();
        for player in &mut self.players {
            let pack = S2cPacket::SyncPlayerList(player.host, playerlist.clone());
            player.send_packet(pack.clone()).await;
        }
    }

    fn player_mut(&mut self, addr: SocketAddr) -> Option<&mut PacketPlayer> {
        self.players.iter_mut().find(|p| p.sock_addr == addr)
    }

    fn player_name(&self, addr: SocketAddr) -> Option<String> {
        self.players
            .iter()
            .find(|p| p.sock_addr == addr)
            .map(|p| p.name.to_string())
    }

    async fn player_joined(&mut self, addr: SocketAddr) {
        println!("joined: {addr}");
    }

    fn addr_from_name(&mut self, name: &str) -> Option<SocketAddr> {
        self.players
            .iter_mut()
            .find(|p| p.name == name)
            .map(|p| p.sock_addr)
    }

    async fn remove_player(&mut self, addr: SocketAddr, reason: Option<String>) {
        let Some(player) = self.player_mut(addr) else {
            return;
        };

        player
            .disconnect(reason.unwrap_or("You have been disconnected".to_string()))
            .await;
        self.players.retain(|p| p.sock_addr != addr);
        self.sync_playerlist().await;
    }

    async fn on_packet(&mut self, addr: SocketAddr, pack: C2sPacket) {
        println!("{addr}: {pack:?}");
        let Some(pl) = self.player_mut(addr) else {
            return;
        };
        let host = pl.host;
        match pack {
            C2sPacket::CreatePlayer(_) => { /* should be handled for us */ }
            C2sPacket::HostStartGame => {
                if !host {
                    self.remove_player(
                        addr,
                        Some("Attempt to send host packet as non-host".to_string()),
                    )
                    .await;
                    return;
                }
                self.game = Some(Game::new(
                    self.players.iter().map(|p| p.name.to_string()).collect(),
                ));
                for pl in &mut self.players {
                    pl.send_packet(S2cPacket::SyncGame(self.game.clone().unwrap()))
                        .await;
                }
            }
            C2sPacket::KeyPress(key) => {
                let name = self.player_name(addr).unwrap_or_default();
                let Some(game) = self.game.as_mut() else {
                    return;
                };

                if game.current() == name {
                    game.progress.insert(key, 0);
                }
            }
            C2sPacket::KeyRelease(key) => {
                let name = self.player_name(addr).unwrap_or_default();
                let Some(game) = self.game.as_mut() else {
                    return;
                };

                if game.current() == name {
                    game.progress.remove(&key);
                }

                for pl in &mut self.players {
                    pl.send_packet(S2cPacket::SyncGame(game.clone())).await;
                }
            }
        }
    }

    async fn tick(&mut self) {
        let Some(mut game) = self.game.clone() else {
            return;
        };
        let addr = self.addr_from_name(&game.current()).unwrap();

        let mut should_update = false;
        let mut fired = None;
        for (key, val) in game.progress.iter_mut() {
            *val += 15;
            should_update = true;
            if *val == 255 {
                fired = Some(*key);
            }
        }

        if let Some(fired) = fired {
            game.progress.clear();
            game.fired.insert(fired);
            game.advance();

            if fired == game.trigger_key {
                self.remove_player(addr, Some("You lost.".to_string()))
                    .await;

                if game.fired.len() >= 26 {
                    game.fired.clear();
                }

                if self.players.len() == 1 {
                    self.remove_player(self.players[0].sock_addr, Some("You won :)".to_string()))
                        .await;
                }
            }
        }

        if should_update {
            for pl in &mut self.players {
                pl.send_packet(S2cPacket::SyncGame(game.clone())).await;
            }
        }
        self.game = Some(game);
    }
}
