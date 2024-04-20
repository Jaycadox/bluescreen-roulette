use std::net::IpAddr;
use std::ops::Deref;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};

use egui_macroquad::egui::{self, RichText};
use egui_macroquad::macroquad::prelude::*;

use crate::lobby::LobbyState;
use crate::server::Server;
use crate::GameState;

enum PubIpResolveStage {
    Waiting(Receiver<Option<String>>),
    Done(String),
}

pub struct MainMenuState {
    ip: Option<String>,
    pub_ip: Arc<Mutex<PubIpResolveStage>>,
    ip_edit: String,
    username_edit: String,
}

impl MainMenuState {
    pub fn new() -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            Self::resolve_pub_ip(tx);
        });

        Self {
            ip: None,
            pub_ip: Arc::new(Mutex::new(PubIpResolveStage::Waiting(rx))),
            ip_edit: String::new(),
            username_edit: String::new(),
        }
    }

    fn resolve_pub_ip(tx: Sender<Option<String>>) {
        let req = ureq::get("http://api.ipify.org");
        let Ok(resp) = req.call() else {
            let _ = tx.send(None);
            return;
        };
        let Ok(resp) = resp.into_string() else {
            let _ = tx.send(None);
            return;
        };
        let _ = tx.send(Some(resp));
    }

    pub fn tick(mut self) -> GameState {
        let ip = self
            .ip
            .clone()
            .unwrap_or_else(|| Self::get_local_ip().unwrap_or("Unknown".to_string()));

        let pub_ip = match self.pub_ip.lock().unwrap().deref() {
            PubIpResolveStage::Waiting(rx) => {
                if let Ok(val) = rx.try_recv() {
                    val
                } else {
                    None
                }
            }
            PubIpResolveStage::Done(text) => Some(text.to_owned()),
        };

        if let Some(pub_ip) = pub_ip.to_owned() {
            *self.pub_ip.lock().unwrap() = PubIpResolveStage::Done(pub_ip);
        }

        let pub_ip = pub_ip.unwrap_or("...".to_string());
        let mut new_gamestate = None;

        egui_macroquad::ui(|ctx| {
            egui::CentralPanel::default()
                .frame(egui::Frame::dark_canvas(&ctx.style()))
                .show(ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.label(RichText::new("Bluescreen Roulette").size(32.0));
                        ui.label(format!(
                            "Your local IP is: {} (if you're playing via LAN)",
                            ip
                        ));
                        ui.label(format!(
                            "Your public IP is: {} (if you're playing via Internet)",
                            pub_ip
                        ));
                    });

                    let window_pos_x = (screen_width() - 200.0) / 2.0;
                    let window_pos_y = (screen_height() - 200.0) / 2.0;

                    egui::Window::new("Play")
                        .fixed_pos((window_pos_x, window_pos_y))
                        .fixed_size((200.0, 200.0))
                        .collapsible(false)
                        .resizable(false)
                        .show(ctx, |ui| {
                            ui.horizontal(|ui| {
                                ui.label("Username");
                                ui.text_edit_singleline(&mut self.username_edit);
                            });
                            ui.separator();

                            ui.horizontal(|ui| {
                                ui.label("IP");
                                ui.text_edit_singleline(&mut self.ip_edit);
                            });
                            if ui.button("Connect to server").clicked() {
                                new_gamestate = Some(LobbyState::try_new(
                                    &self.username_edit,
                                    &format!("{}:1234", self.ip_edit),
                                ));
                            }
                            ui.separator();
                            ui.horizontal(|ui| {
                                ui.label("or");
                                if ui.button("Host server").clicked() {
                                    tokio::spawn(async move {
                                        Server::start().await;
                                    });
                                    new_gamestate = Some(LobbyState::try_new(
                                        &self.username_edit,
                                        "127.0.0.1:1234",
                                    ));
                                }
                            });
                        });
                });
        });
        egui_macroquad::draw();

        if let Some(new_gamestate) = new_gamestate {
            new_gamestate
        } else {
            GameState::MainMenu(self)
        }
    }
    fn get_local_ip() -> Option<String> {
        if let Ok(ifaces) = get_if_addrs::get_if_addrs() {
            for iface in ifaces {
                if iface.name.starts_with("vir") {
                    continue;
                }

                // Check if the interface is up and not a loopback interface
                if !iface.is_loopback() {
                    match iface.ip() {
                        IpAddr::V4(v4) => {
                            return Some(v4.to_string());
                        }

                        IpAddr::V6(_) => {}
                    }
                }
            }
        }
        None
    }
}
