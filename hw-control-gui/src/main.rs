mod app_ui;
mod socket_client;

use app_ui::AppUi;
use eframe::egui;

fn main() -> Result<(), eframe::Error> {
    // Initialize GUI logger
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    // Set window NativeOptions
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([740.0, 560.0])
            .with_min_inner_size([650.0, 500.0])
            .with_resizable(true),
        ..Default::default()
    };

    eframe::run_native(
        "Linux Hardware Controller",
        options,
        Box::new(|_cc| {
            // Customize look/feel if needed, then return app state
            Box::new(AppUi::new())
        }),
    )
}
