//! GUI-редактор (eframe/egui). M0 — пустое окно-каркас; превью, таймлайн,
//! транспорт и cpal-вывод появляются в M4–M5.

use eframe::egui;

struct KtApp;

impl eframe::App for KtApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Panel::bottom("timeline").resizable(true).default_size(220.0).show(ui, |ui| {
            ui.centered_and_justified(|ui| {
                ui.weak("Таймлайн (M4)");
            });
        });
        egui::CentralPanel::default().show(ui, |ui| {
            ui.centered_and_justified(|ui| {
                ui.weak("Превью (M4)");
            });
        });
    }
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 800.0])
            .with_title("Kinotrain"),
        ..Default::default()
    };
    eframe::run_native("kinotrain", options, Box::new(|_cc| Ok(Box::new(KtApp))))
}
