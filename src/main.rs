// Hide the console window in release builds so the widget launches cleanly.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use chrono::Local;
use eframe::egui::{
    self,
    Pos2, Rect, Vec2,
};


mod theme;
mod utils;
mod store;
mod autostart;
mod app;
mod widgets;
mod todo;
mod tray;

pub use theme::*;
pub use utils::*;
pub use store::*;
pub use autostart::*;
pub use app::*;
pub use widgets::*;
pub use todo::*;
pub use tray::*;

fn main() -> eframe::Result<()> {
    // Release builds have no console, so record any crash next to the notes file.
    std::panic::set_hook(Box::new(|info| {
        let log = data_path().with_file_name("crash.log");
        if let Some(dir) = log.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let line = format!("[{}] {info}\n", Local::now().format("%Y-%m-%d %H:%M:%S"));
        let _ = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log)
            .and_then(|mut f| std::io::Write::write_all(&mut f, line.as_bytes()));
    }));

    let store = load_store();

    // Place the window at its saved dock position before it first appears.
    let wa = work_area_points(system_scale())
        .unwrap_or_else(|| Rect::from_min_size(Pos2::ZERO, Vec2::new(1920.0, 1080.0)));
    let notch = store.edge.notch_size();
    let start = docked_pos(store.edge, anchor_or_center(&store, wa), notch, wa);

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Nothing But Notes")
            .with_decorations(false) // frameless
            .with_transparent(true) // rounded corners need a see-through window
            .with_always_on_top() // floats above other windows like a notch
            .with_taskbar(true) // visible on taskbar and allows minimizing to taskbar
            .with_resizable(false)
            .with_inner_size(notch)
            .with_position(start),
        ..Default::default()
    };

    let res = eframe::run_native(
        "Nothing But Notes",
        options,
        Box::new(|cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            setup_fonts(&cc.egui_ctx);
            let store = store;
            let theme = Theme::get(store.dark_mode);
            setup_style(&cc.egui_ctx, &theme);
            Box::new(WidgetApp::new(store, cc.egui_ctx.clone()))
        }),
    );
    if let Err(ref e) = res {
        let log = data_path().with_file_name("crash.log");
        let _ = std::fs::write(log, format!("[run_native error] {e:?}\n"));
    }
    res
}

