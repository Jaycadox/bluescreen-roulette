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
use lobby::LobbyState;
use main_menu::MainMenuState;

enum GameState {
    MainMenu(MainMenuState),
    Lobby(LobbyState),
    Kicked(KickedState),
    InGame(ClientGameState),
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
