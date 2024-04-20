use egui_macroquad::{
    egui::{self, RichText},
    macroquad::prelude::*,
};
use tokio::{
    net::TcpStream,
    sync::mpsc::{Receiver, Sender},
};

use crate::{
    c2s_packet::C2sPacket, client_game::ClientGameState, kicked::KickedState,
    main_menu::MainMenuState, packet_channel, s2c_packet::S2cPacket, GameState,
};
use anyhow::Result;

pub struct LobbyState {
    tx: Sender<C2sPacket>,
    rx: Receiver<Result<S2cPacket>>,
    username: String,
    players: Vec<String>,
    host: bool,
}

impl LobbyState {
    pub fn try_new(username: &str, ip: &str) -> GameState {
        let ip = ip.to_string();
        let username = username.to_string();
        let (tx, rx) = std::sync::mpsc::channel();
        tokio::spawn(async move {
            match tokio::time::timeout(tokio::time::Duration::from_secs(3), TcpStream::connect(ip))
                .await
            {
                Ok(Ok(stream)) => {
                    let (nrx, ntx) = packet_channel::async_channel(stream);
                    ntx.send(C2sPacket::CreatePlayer(username.to_string()))
                        .await
                        .unwrap();
                    tx.send(GameState::Lobby(Self {
                        tx: ntx,
                        rx: nrx,
                        players: vec![],
                        username: username.to_string(),
                        host: false,
                    }))
                    .unwrap();
                }
                Err(e) => {
                    println!("Error while connecting to server: {e}");
                    tx.send(GameState::MainMenu(MainMenuState::new())).unwrap();
                }
                Ok(Err(e)) => {
                    println!("Error while connecting to server: {e}");
                    tx.send(GameState::MainMenu(MainMenuState::new())).unwrap();
                }
            }
        });
        rx.recv().unwrap()
    }

    fn handle_packet(mut self, pack: S2cPacket) -> (Option<Self>, Option<GameState>) {
        match pack {
            S2cPacket::SyncPlayerList(host, list) => {
                self.host = host;
                self.players = list;
                (Some(self), None)
            }
            S2cPacket::Disconnect(msg) => {
                println!("Kicked: {msg}");
                (
                    Some(self),
                    Some(GameState::Kicked(KickedState { message: msg })),
                )
            }
            S2cPacket::SyncGame(game) => {
                let client_game = ClientGameState {
                    game,
                    tx: self.tx,
                    rx: self.rx,
                    players: self.players,
                    host: self.host,
                    username: self.username,
                };
                (None, Some(GameState::InGame(client_game)))
            }
            S2cPacket::KillYourselfNow => {
                /* should not occur until game start */
                (Some(self), None)
            }
            S2cPacket::PlaySound(_) => (Some(self), None),
        }
    }

    pub async fn tick(mut self) -> GameState {
        let mut new_game_state = None;

        if let Ok(Ok(packet)) = self.rx.try_recv() {
            let (new_self, new_state) = self.handle_packet(packet);
            if let Some(new_state) = new_state {
                return new_state;
            }
            self = new_self.unwrap();
        }
        let mut should_start_game = false;

        egui_macroquad::ui(|ctx| {
            egui::CentralPanel::default()
                .frame(egui::Frame::dark_canvas(&ctx.style()))
                .show(ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.label(RichText::new("Bluescreen Roulette").size(32.0));
                        let window_pos_x = (screen_width() - 200.0) / 2.0;
                        let window_pos_y = (screen_height() - 200.0) / 2.0;

                        egui::Window::new("Lobby")
                            .fixed_pos((window_pos_x, window_pos_y))
                            .fixed_size((200.0, 200.0))
                            .collapsible(false)
                            .resizable(false)
                            .show(ctx, |ui| {
                                egui::Grid::new("list")
                                    .striped(true)
                                    .min_col_width(200.0)
                                    .show(ui, |ui| {
                                        for player in &self.players {
                                            ui.label(player);
                                            ui.end_row();
                                        }
                                    });
                                ui.horizontal(|ui| {
                                    if ui.button("Leave").clicked() {
                                        new_game_state =
                                            Some(GameState::MainMenu(MainMenuState::new()));
                                    }

                                    if self.host && ui.button("Start").clicked() {
                                        should_start_game = true;
                                    }
                                });
                            });
                    });
                });
        });

        if should_start_game {
            let _ = self.tx.send(C2sPacket::HostStartGame).await;
        }

        egui_macroquad::draw();

        if let Some(new_game_state) = new_game_state {
            new_game_state
        } else {
            GameState::Lobby(self)
        }
    }
}
