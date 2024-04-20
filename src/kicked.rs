use egui_macroquad::{
    egui::{self, RichText},
    macroquad::prelude::*,
};

use crate::{main_menu::MainMenuState, GameState};

pub struct KickedState {
    pub message: String,
}

impl KickedState {
    pub fn tick(self) -> GameState {
        let mut new_game_state = None;
        egui_macroquad::ui(|ctx| {
            egui::CentralPanel::default()
                .frame(egui::Frame::dark_canvas(&ctx.style()))
                .show(ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.label(RichText::new("Bluescreen Roulette").size(32.0));
                        let window_pos_x = (screen_width() - 200.0) / 2.0;
                        let window_pos_y = (screen_height() - 200.0) / 2.0;

                        egui::Window::new("Disconnected")
                            .fixed_pos((window_pos_x, window_pos_y))
                            .fixed_size((200.0, 200.0))
                            .collapsible(false)
                            .resizable(false)
                            .show(ctx, |ui| {
                                ui.label(&self.message);
                                if ui.button("Ok").clicked() {
                                    new_game_state =
                                        Some(GameState::MainMenu(MainMenuState::new()));
                                }
                            });
                    });
                });
        });
        egui_macroquad::draw();

        if let Some(new_game_state) = new_game_state {
            new_game_state
        } else {
            GameState::Kicked(self)
        }
    }
}
