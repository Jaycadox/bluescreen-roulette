use std::collections::HashMap;
use std::io::Cursor;

use client_game::ClientGameState;
use egui_macroquad::macroquad;
use egui_macroquad::macroquad::prelude::*;
use egui_macroquad::macroquad::window::clear_background;

mod c2s_packet;
mod client_game;
mod kicked;
mod lobby;
mod main_menu;
mod packet_channel;
mod s2c_packet;
mod server;
use kicked::KickedState;
use lazy_static::lazy_static;
use lobby::LobbyState;
use main_menu::MainMenuState;

enum GameState {
    MainMenu(MainMenuState),
    Lobby(LobbyState),
    Kicked(KickedState),
    InGame(ClientGameState),
}

lazy_static! {
    static ref BUNDLE: HashMap<String, Vec<u8>> = {
        let mut m = HashMap::new();
        let bundle = include_bytes!("bundle.pfa");

        let mut reader = pfa::reader::PfaReader::new(Cursor::new(bundle)).unwrap();
        reader.traverse_files("/", |file| {
            println!("[Bundle] file={}", file.get_path());
            m.insert(file.get_path().to_string(), file.get_contents().to_vec());
        });

        m
    };
}

#[tokio::main]
async fn main() {
    macroquad::Window::new("Bluescreen Roulette", async move {
        let mut game_state = GameState::MainMenu(MainMenuState::new());
        loop {
            clear_background(BLACK);
            match game_state {
                GameState::MainMenu(main_menu) => {
                    game_state = main_menu.tick();
                }
                GameState::Lobby(lobby) => {
                    game_state = lobby.tick().await;
                }
                GameState::Kicked(kicked) => {
                    game_state = kicked.tick();
                }
                GameState::InGame(game) => {
                    game_state = game.tick().await;
                }
            };

            next_frame().await;
        }
    });
}
