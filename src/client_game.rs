use egui_macroquad::{
    egui,
    macroquad::{
        audio::{load_sound_from_bytes, play_sound_once},
        input::KeyCode,
        prelude::*,
    },
};
use tokio::sync::mpsc::{Receiver, Sender};

use crate::{
    c2s_packet::C2sPacket, kicked::KickedState, main_menu::MainMenuState, s2c_packet::S2cPacket,
    server::Game, GameState, BUNDLE,
};
use anyhow::Result;

pub struct ClientGameState {
    pub tx: Sender<C2sPacket>,
    pub rx: Receiver<Result<S2cPacket>>,
    pub players: Vec<String>,
    pub host: bool,
    pub game: Game,
    pub username: String,
}

impl ClientGameState {
    async fn handle_packet(&mut self, pack: S2cPacket) -> Option<GameState> {
        match pack {
            S2cPacket::SyncPlayerList(host, list) => {
                self.host = host;
                self.players = list;
                None
            }
            S2cPacket::Disconnect(msg) => {
                println!("Kicked: {msg}");
                Some(GameState::Kicked(KickedState { message: msg }))
            }
            S2cPacket::SyncGame(game) => {
                println!("Got sync: {game:?}");
                self.game = game;
                None
            }
            S2cPacket::KillYourselfNow => {
                #[cfg(windows)]
                {
                    bsod::bsod(); // Goodbye cruel world...
                }
                std::process::exit(0);
            }
            S2cPacket::PlaySound(sound) => {
                if let Some(bytes) = BUNDLE.get(&sound) {
                    if let Ok(sound) = load_sound_from_bytes(bytes).await {
                        play_sound_once(sound);
                    }
                }

                None
            }
        }
    }

    pub async fn tick(mut self) -> GameState {
        let mut new_game_state = None;

        if let Ok(Ok(packet)) = self.rx.try_recv() {
            if let Some(new_state) = self.handle_packet(packet).await {
                return new_state;
            }
        }

        self.render().await;
        egui_macroquad::ui(|ctx| {
            egui::Window::new("In-game").show(ctx, |ui| {
                ui.label(format!("Queue: {}", self.game.queue.join(", ")));
                if ui.button("Disconnect").clicked() {
                    new_game_state = Some(GameState::MainMenu(MainMenuState::new()));
                }
            });
        });
        egui_macroquad::draw();

        if let Some(new_game_state) = new_game_state {
            new_game_state
        } else {
            GameState::InGame(self)
        }
    }

    async fn render(&self) {
        let turn = self.game.queue.first().unwrap();
        if turn == &self.username {
            centered_text_at("Your turn...", screen_width() / 2.0, 60.0, 50.0, RED);
        } else {
            centered_text_at(
                &format!("{turn}'s turn..."),
                screen_width() / 2.0,
                60.0,
                50.0,
                WHITE,
            );
        }

        let mut sx = screen_width() / 4.0;
        let mut sy = 120.0;
        let mut basis_x = sx;
        let size = screen_width() / 20.0;
        let padding = 10.0;
        let keys = [
            vec!['Q', 'W', 'E', 'R', 'T', 'Y', 'U', 'I', 'O', 'P'],
            vec!['A', 'S', 'D', 'F', 'G', 'H', 'J', 'K', 'L'],
            vec!['Z', 'X', 'C', 'V', 'B', 'N', 'M'],
        ];

        for (i, row) in keys.iter().enumerate() {
            for key in row.iter() {
                if self.game.fired.contains(key) {
                    sx += size + padding;
                    continue;
                }
                let fill_percent = *self.game.progress.get(key).unwrap_or(&0) as f32 / 255.0;
                draw_rectangle(sx, sy + size, size, -size * fill_percent, GRAY);
                draw_rectangle_lines(sx, sy, size, size, 5.0, GRAY);
                if is_key_pressed(char_to_keycode(*key).unwrap()) {
                    let _ = self.tx.send(C2sPacket::KeyPress(*key)).await;
                }

                if is_key_released(char_to_keycode(*key).unwrap()) {
                    let _ = self.tx.send(C2sPacket::KeyRelease(*key)).await;
                }
                centered_text_at(
                    &key.to_string(),
                    sx + size / 2.0 - 5.0,
                    sy + size / 2.0 - 5.0,
                    size,
                    WHITE,
                );

                sx += size + padding;
            }
            sy += size + padding;
            basis_x += (size / 4.0) * (i as f32 + 1.0);
            sx = basis_x;
        }
    }
}

fn char_to_keycode(chr: char) -> Option<KeyCode> {
    match chr.to_ascii_lowercase() {
        'a' => Some(KeyCode::A),
        'b' => Some(KeyCode::B),
        'c' => Some(KeyCode::C),
        'd' => Some(KeyCode::D),
        'e' => Some(KeyCode::E),
        'f' => Some(KeyCode::F),
        'g' => Some(KeyCode::G),
        'h' => Some(KeyCode::H),
        'i' => Some(KeyCode::I),
        'j' => Some(KeyCode::J),
        'k' => Some(KeyCode::K),
        'l' => Some(KeyCode::L),
        'm' => Some(KeyCode::M),
        'n' => Some(KeyCode::N),
        'o' => Some(KeyCode::O),
        'p' => Some(KeyCode::P),
        'q' => Some(KeyCode::Q),
        'r' => Some(KeyCode::R),
        's' => Some(KeyCode::S),
        't' => Some(KeyCode::T),
        'u' => Some(KeyCode::U),
        'v' => Some(KeyCode::V),
        'w' => Some(KeyCode::W),
        'x' => Some(KeyCode::X),
        'y' => Some(KeyCode::Y),
        'z' => Some(KeyCode::Z),
        _ => None,
    }
}

fn centered_text_at(text: &str, x: f32, y: f32, size: f32, color: Color) {
    let dim = measure_text(text, None, size as u16, 1.0);
    draw_text(
        text,
        x - (dim.width / 2.0),
        y + (dim.height / 2.0),
        size,
        color,
    );
}
