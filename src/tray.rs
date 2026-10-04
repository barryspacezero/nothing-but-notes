use tray_icon::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent,
};
use tray_icon::menu::MenuEvent;

pub struct TrayState {
    pub tray_icon: TrayIcon,
    pub item_open: MenuItem,
    pub item_new_note: MenuItem,
    pub item_new_todo: MenuItem,
    pub item_guide: MenuItem,
    pub item_minimize: MenuItem,
    pub item_quit: MenuItem,
}

pub fn create_tray() -> Option<TrayState> {
    let icon = load_icon()?;
    let menu = Menu::new();

    let item_open = MenuItem::new("Open Nothing But Notes", true, None);
    let item_new_note = MenuItem::new("New Note", true, None);
    let item_new_todo = MenuItem::new("New To-Do", true, None);
    let item_guide = MenuItem::new("Features Guide", true, None);
    let item_minimize = MenuItem::new("Minimize", true, None);
    let item_quit = MenuItem::new("Quit", true, None);

    let _ = menu.append(&item_open);
    let _ = menu.append(&item_new_note);
    let _ = menu.append(&item_new_todo);
    let _ = menu.append(&item_guide);
    let _ = menu.append(&item_minimize);
    let _ = menu.append(&PredefinedMenuItem::separator());
    let _ = menu.append(&item_quit);

    let tray_icon = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("Nothing But Notes")
        .with_icon(icon)
        .build()
        .ok()?;

    Some(TrayState {
        tray_icon,
        item_open,
        item_new_note,
        item_new_todo,
        item_guide,
        item_minimize,
        item_quit,
    })
}

pub enum TrayAction {
    Open,
    NewNote,
    NewTodo,
    Guide,
    Minimize,
    Quit,
}

pub fn handle_tray_events(tray: &TrayState) -> Option<TrayAction> {
    while let Ok(event) = MenuEvent::receiver().try_recv() {
        if event.id == tray.item_open.id() {
            return Some(TrayAction::Open);
        } else if event.id == tray.item_new_note.id() {
            return Some(TrayAction::NewNote);
        } else if event.id == tray.item_new_todo.id() {
            return Some(TrayAction::NewTodo);
        } else if event.id == tray.item_guide.id() {
            return Some(TrayAction::Guide);
        } else if event.id == tray.item_minimize.id() {
            return Some(TrayAction::Minimize);
        } else if event.id == tray.item_quit.id() {
            return Some(TrayAction::Quit);
        }
    }

    while let Ok(event) = TrayIconEvent::receiver().try_recv() {
        match event {
            TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } => {
                return Some(TrayAction::Open);
            }
            TrayIconEvent::DoubleClick { button: MouseButton::Left, .. } => {
                return Some(TrayAction::Open);
            }
            _ => {}
        }
    }

    None
}

pub fn load_icon() -> Option<Icon> {
    if let Ok(img) = image::load_from_memory_with_format(include_bytes!("../icon.ico"), image::ImageFormat::Ico) {
        let rgba = img.into_rgba8();
        let (w, h) = rgba.dimensions();
        if let Ok(icon) = Icon::from_rgba(rgba.into_raw(), w, h) {
            return Some(icon);
        }
    }

    // Fallback: 32x32 Nothing-style icon (crisp dark circle with signature red dot)
    let w = 32u32;
    let h = 32u32;
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let dx = x as f32 - 15.5;
            let dy = y as f32 - 15.5;
            let dist = (dx * dx + dy * dy).sqrt();
            if dist <= 4.5 {
                // Signature Nothing red dot: #EB2D2D
                rgba.extend_from_slice(&[235, 45, 45, 255]);
            } else if dist <= 14.5 {
                // Dark pill/circle background
                rgba.extend_from_slice(&[22, 22, 22, 255]);
            } else if dist <= 15.5 {
                // Antialiased border
                rgba.extend_from_slice(&[45, 45, 45, 180]);
            } else {
                rgba.extend_from_slice(&[0, 0, 0, 0]);
            }
        }
    }
    Icon::from_rgba(rgba, w, h).ok()
}
