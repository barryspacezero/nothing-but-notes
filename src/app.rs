use chrono::{Local, NaiveDate};
use eframe::egui::{self, Align2, Color32, CursorIcon, FontId, Key, Pos2, Rect, Response, RichText, Rounding, Sense, Stroke, Vec2};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, hotkey::{HotKey, Modifiers, Code}};
use arboard::Clipboard;
use crossbeam_channel::Receiver;
use std::time::Instant;

use crate::*;

// ── App ─────────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
pub enum View {
    List,
    Edit(u64),
}

/// Right-click actions on a note in the list.
#[derive(Clone, Copy)]
enum NoteOp {
    Open(u64),
    Pin(u64),
    Duplicate(u64),
    Copy(u64),
    ToList(u64),
    ToText(u64),
    Delete(u64),
    NewNote,
    NewList,
    ClearClipboard,
}

pub struct WidgetApp {
    pub store: Store,
    pub view: View,
    pub search: String,
    pub expanded: bool,
    pub was_focused: bool,
    /// Set when a note opens so the editor grabs keyboard focus once.
    pub focus_editor: bool,
    pub confirm_delete: bool,
    /// Checklist row that should receive keyboard focus next frame.
    pub todo_focus: Option<usize>,
    /// Cache the parsed checklist so we don't allocate strings every frame!
    pub open_todo: Option<Vec<crate::todo::Item>>,

    /// Collapsed size, smoothed so switching horizontal ↔ vertical morphs.
    pub notch: Vec2,
    /// Offset from the docked position that decays to zero after a drop (snap animation).
    pub snap_offset: Vec2,
    /// Pointer position inside the window when a drag began.
    pub drag_grab: Option<Vec2>,
    /// Window position while dragging.
    pub drag_pos: Pos2,
    pub sent_pos: Option<Pos2>,
    pub sent_size: Option<Vec2>,
    pub dirty_since: Option<Instant>,

    pub hotkey_manager: GlobalHotKeyManager,
    pub hotkey_id: Option<u32>,
    pub hotkey_label: &'static str,
    pub clipboard_rx: Receiver<String>,
    /// Text we copied ourselves — the clipboard watcher must not turn it into a note.
    pub ignore_clip: Option<String>,
}

pub fn spawn_clipboard_thread() -> Receiver<String> {
    let (tx, rx) = crossbeam_channel::unbounded();
    std::thread::spawn(move || {
        let mut last_text = String::new();
        if let Ok(mut ctx) = Clipboard::new() {
            // Seed with the current clipboard so startup doesn't create a note.
            if let Ok(t) = ctx.get_text() {
                last_text = t.trim().to_owned();
            }
            loop {
                if let Ok(text) = ctx.get_text() {
                    let trimmed = text.trim();
                    if !trimmed.is_empty() && trimmed != last_text {
                        last_text = trimmed.to_owned();
                        let _ = tx.send(last_text.clone());
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(1000));
            }
        }
    });
    rx
}

/// Alt+Space is taken by ChatGPT / PowerToys Run, so try a few free combos in order.
fn register_hotkey(manager: &GlobalHotKeyManager) -> (Option<u32>, &'static str) {
    let candidates = [
        (HotKey::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyN), "CTRL+ALT+N"),
        (HotKey::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::Space), "CTRL+SHIFT+SPACE"),
        (HotKey::new(Some(Modifiers::ALT | Modifiers::SHIFT), Code::KeyN), "ALT+SHIFT+N"),
    ];
    for (hk, label) in candidates {
        if manager.register(hk).is_ok() {
            return (Some(hk.id()), label);
        }
    }
    (None, "NO HOTKEY")
}

impl WidgetApp {
    pub fn new(store: Store) -> Self {
        let notch = store.edge.notch_size();
        // Sync registry state on startup
        set_autostart_registry(store.autostart);

        let hotkey_manager = GlobalHotKeyManager::new().unwrap();
        let (hotkey_id, hotkey_label) = register_hotkey(&hotkey_manager);

        let clipboard_rx = spawn_clipboard_thread();

        Self {
            store,
            view: View::List,
            search: String::new(),
            expanded: false,
            was_focused: false,
            focus_editor: false,
            confirm_delete: false,
            todo_focus: None,
            open_todo: None,
            notch,
            snap_offset: Vec2::ZERO,
            drag_grab: None,
            drag_pos: Pos2::ZERO,
            sent_pos: None,
            sent_size: None,
            dirty_since: None,
            hotkey_manager,
            hotkey_id,
            hotkey_label,
            clipboard_rx,
            ignore_clip: None,
        }
    }
}

impl eframe::App for WidgetApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        egui::Rgba::TRANSPARENT.to_array() // corners outside the shape stay see-through
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let dt = ctx.input(|i| i.stable_dt).min(0.05);

        let theme = Theme::get(self.store.dark_mode);
        setup_style(ctx, &theme);

        // Clicking anywhere outside the widget collapses it.
        let focused = ctx.input(|i| i.focused);
        if self.was_focused && !focused {
            self.collapse();
        }
        self.was_focused = focused;

        self.handle_shortcuts(ctx);

        while let Ok(event) = GlobalHotKeyEvent::receiver().try_recv() {
            let ours = self.hotkey_id.map_or(true, |id| id == event.id);
            if ours && event.state == global_hotkey::HotKeyState::Pressed {
                if !self.expanded {
                    self.expanded = true;
                    self.new_note();
                } else {
                    self.collapse();
                }
                ctx.request_repaint();
                ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
            }
        }
        // The hotkey and clipboard arrive from other threads; keep polling lightly.
        ctx.request_repaint_after(std::time::Duration::from_millis(250));

        while let Ok(text) = self.clipboard_rx.try_recv() {
            if self.ignore_clip.as_deref() == Some(text.as_str()) {
                self.ignore_clip = None;
                continue;
            }
            if self.store.notes.iter().any(|n| n.text.trim() == text) {
                continue;
            }
            let id = self.alloc_id();
            self.store.notes.push(Note::new(id, text, true));
            self.save_now();
            ctx.request_repaint();
        }

        if !ctx.input(|i| i.raw.dropped_files.is_empty()) {
            for file in ctx.input(|i| i.raw.dropped_files.clone()) {
                if let Some(path) = file.path {
                    let media_dir = data_path().with_file_name("media");
                    let _ = std::fs::create_dir_all(&media_dir);
                    if let Some(name) = path.file_name() {
                        let dest = media_dir.join(name);
                        let _ = std::fs::copy(&path, &dest);
                        let text = format!("![{}]({})", name.to_string_lossy(), dest.to_string_lossy());
                        let id = self.alloc_id();
                        self.store.notes.push(Note::new(id, text, false));
                        self.save_now();
                    }
                }
            }
        }

        // 0.0 = notch, 1.0 = fully expanded panel.
        let t = ctx.animate_bool_with_time(egui::Id::new("expand"), self.expanded, ANIM_TIME);
        let e = ease_out_cubic(t);

        // Smoothly morph the notch orientation and settle the snap after a drop.
        let k = 1.0 - (-dt * 16.0).exp();
        let target_notch = self.store.edge.notch_size();
        if (target_notch - self.notch).length() > 0.5 {
            self.notch += (target_notch - self.notch) * k;
            ctx.request_repaint();
        } else {
            self.notch = target_notch;
        }
        if self.snap_offset.length() > 0.5 {
            self.snap_offset *= 1.0 - k;
            ctx.request_repaint();
        } else {
            self.snap_offset = Vec2::ZERO;
        }

        let size = self.notch + (PANEL_SIZE - self.notch) * e;
        let dragging = self.drag_grab.is_some();

        egui::CentralPanel::default()
            .frame(egui::Frame::none())
            .show(ctx, |ui| {
                let rect = ctx.screen_rect();
                let r = lerp(self.notch.x.min(self.notch.y) / 2.0, PANEL_RADIUS, e);
                let rounding = if dragging { Rounding::same(r) } else { self.store.edge.rounding(r) };

                ui.painter().rect_filled(rect, rounding, theme.bg);
                ui.painter().rect_stroke(rect.shrink(0.5), rounding, Stroke::new(1.0, theme.border));

                if t > 0.9 {
                    self.draw_panel(ui, ctx, rect, &theme);
                } else if t < 0.1 {
                    self.draw_notch(ui, ctx, rect, &theme);
                }
            });

        self.place_window(ctx, size);
        self.autosave(ctx);

        if ctx.memory(|m| m.focused().is_some()) {
            ctx.request_repaint(); // continuously repaint when typing so cursor blinks
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.cleanup_empty();
        self.save_now();
    }
}

// ── Window placement & dragging ─────────────────────────────────────────────

impl WidgetApp {
    pub fn work_area(&self, ctx: &egui::Context) -> Rect {
        let ppp = ctx.input(|i| i.viewport().native_pixels_per_point).unwrap_or(1.0);
        work_area_points(ppp)
            .or_else(|| ctx.input(|i| i.viewport().monitor_size).map(|s| Rect::from_min_size(Pos2::ZERO, s)))
            .unwrap_or_else(|| Rect::from_min_size(Pos2::ZERO, Vec2::new(1920.0, 1080.0)))
    }

    pub fn place_window(&mut self, ctx: &egui::Context, size: Vec2) {
        let size = Vec2::new(size.x.round(), size.y.round());
        let pos = if self.drag_grab.is_some() {
            self.drag_pos
        } else {
            let wa = self.work_area(ctx);
            docked_pos(self.store.edge, anchor_or_center(&self.store, wa), size, wa) + self.snap_offset
        };
        let pos = Pos2::new(pos.x.round(), pos.y.round());

        if self.sent_size != Some(size) {
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(size));
            self.sent_size = Some(size);
        }
        if self.sent_pos != Some(pos) {
            ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(pos));
            self.sent_pos = Some(pos);
        }
    }

    /// Move the window with the pointer; snap to the nearest edge on release.
    pub fn handle_drag(&mut self, ctx: &egui::Context, resp: &Response) {
        let outer = ctx
            .input(|i| i.viewport().outer_rect)
            .map(|r| r.min)
            .or(self.sent_pos)
            .unwrap_or(Pos2::ZERO);

        if resp.drag_started() {
            if let Some(p) = resp.interact_pointer_pos() {
                self.drag_grab = Some(p.to_vec2());
                self.drag_pos = outer;
                self.snap_offset = Vec2::ZERO;
            }
        }

        if let Some(grab) = self.drag_grab {
            ctx.set_cursor_icon(CursorIcon::Grabbing);
            if resp.dragged() {
                if let Some(p) = ctx.input(|i| i.pointer.interact_pos()) {
                    // Global pointer = window origin + local pointer.
                    self.drag_pos = outer + p.to_vec2() - grab;
                }
            }
            let released = resp.drag_stopped() || !ctx.input(|i| i.pointer.any_down());
            if released {
                self.drop_window(ctx);
            }
        } else if resp.hovered() {
            ctx.set_cursor_icon(CursorIcon::Grab);
        }
    }

    pub fn drop_window(&mut self, ctx: &egui::Context) {
        self.drag_grab = None;
        let wa = self.work_area(ctx);
        let size = self.sent_size.unwrap_or(self.notch);
        let c = self.drag_pos + size / 2.0;

        let edge = [
            (Edge::Top, c.y - wa.top()),
            (Edge::Bottom, wa.bottom() - c.y),
            (Edge::Left, c.x - wa.left()),
            (Edge::Right, wa.right() - c.x),
        ]
        .into_iter()
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(edge, _)| edge)
        .unwrap_or_default();

        let anchor = if edge.vertical() { c.y } else { c.x };
        self.store.edge = edge;
        self.store.anchor = Some(anchor);
        self.snap_offset = self.drag_pos - docked_pos(edge, anchor, size, wa);
        self.mark_dirty();
    }
}

// ── Notes logic ─────────────────────────────────────────────────────────────

impl WidgetApp {
    pub fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        if !self.expanded {
            return;
        }
        let (esc, new, list) = ctx.input(|i| {
            (
                i.key_pressed(Key::Escape),
                i.modifiers.ctrl && i.key_pressed(Key::N),
                i.modifiers.ctrl && i.key_pressed(Key::T),
            )
        });
        if new {
            self.new_note();
        } else if list {
            self.new_list();
        } else if esc {
            match self.view {
                View::Edit(_) => self.leave_editor(),
                View::List => self.collapse(),
            }
        }
    }

    pub fn alloc_id(&mut self) -> u64 {
        let id = self
            .store
            .next_id
            .max(self.store.notes.iter().map(|n| n.id + 1).max().unwrap_or(0));
        self.store.next_id = id + 1;
        id
    }

    pub fn collapse(&mut self) {
        self.expanded = false;
        self.confirm_delete = false;
        self.cleanup_empty();
        self.save_now();
    }

    pub fn new_note(&mut self) {
        self.cleanup_empty();
        let id = self.alloc_id();
        self.store.notes.push(Note::new(id, String::new(), false));
        self.search.clear();
        self.open_note(id);
    }

    pub fn new_list(&mut self) {
        self.cleanup_empty();
        let id = self.alloc_id();
        self.store.notes.push(Note::new(id, "- [ ] ".to_owned(), false));
        self.search.clear();
        self.open_note(id);
        self.todo_focus = Some(0);
    }

    pub fn open_note(&mut self, id: u64) {
        self.view = View::Edit(id);
        self.focus_editor = true;
        self.confirm_delete = false;
        if let Some(note) = self.store.notes.iter().find(|n| n.id == id) {
            if note.is_todo() {
                self.open_todo = Some(crate::todo::parse_items(&note.text));
            } else {
                self.open_todo = None;
            }
        }
    }

    pub fn leave_editor(&mut self) {
        self.view = View::List;
        self.confirm_delete = false;
        self.todo_focus = None;
        self.open_todo = None;
        self.cleanup_empty();
    }

    /// Drop notes that were opened but never written in.
    pub fn cleanup_empty(&mut self) {
        let before = self.store.notes.len();
        self.store.notes.retain(|n| !n.is_blank());
        if self.store.notes.len() != before {
            self.mark_dirty();
        }
        if let View::Edit(id) = self.view {
            if !self.store.notes.iter().any(|n| n.id == id) {
                self.view = View::List;
            }
        }
    }

    /// Copy to the system clipboard without the watcher re-importing it as a note.
    pub fn copy_text(&mut self, ctx: &egui::Context, text: String) {
        self.ignore_clip = Some(text.trim().to_owned());
        ctx.output_mut(|o| o.copied_text = text);
    }

    pub fn mark_dirty(&mut self) {
        self.dirty_since = Some(Instant::now());
    }

    pub fn autosave(&mut self, ctx: &egui::Context) {
        if let Some(t) = self.dirty_since {
            let elapsed = t.elapsed();
            if elapsed >= SAVE_DELAY {
                self.save_now();
            } else {
                ctx.request_repaint_after(SAVE_DELAY - elapsed);
            }
        }
    }

    pub fn save_now(&mut self) {
        self.dirty_since = None;
        let _ = save_store(&self.store);
    }

    fn apply_note_op(&mut self, op: NoteOp, ctx: &egui::Context) {
        let find = |s: &Self, id: u64| s.store.notes.iter().position(|n| n.id == id);
        match op {
            NoteOp::Open(id) => self.open_note(id),
            NoteOp::Pin(id) => {
                if let Some(i) = find(self, id) {
                    self.store.notes[i].pinned = !self.store.notes[i].pinned;
                }
            }
            NoteOp::Duplicate(id) => {
                if let Some(i) = find(self, id) {
                    let text = self.store.notes[i].text.clone();
                    let new_id = self.alloc_id();
                    self.store.notes.push(Note::new(new_id, text, false));
                }
            }
            NoteOp::Copy(id) => {
                if let Some(i) = find(self, id) {
                    let text = list_to_text(&self.store.notes[i].text);
                    self.copy_text(ctx, text);
                }
                return;
            }
            NoteOp::ToList(id) => {
                if let Some(i) = find(self, id) {
                    self.store.notes[i].text = text_to_list(&self.store.notes[i].text);
                }
            }
            NoteOp::ToText(id) => {
                if let Some(i) = find(self, id) {
                    self.store.notes[i].text = list_to_text(&self.store.notes[i].text);
                }
            }
            NoteOp::Delete(id) => self.store.notes.retain(|n| n.id != id),
            NoteOp::NewNote => return self.new_note(),
            NoteOp::NewList => return self.new_list(),
            NoteOp::ClearClipboard => self.store.notes.retain(|n| !n.is_clipboard || n.pinned),
        }
        self.mark_dirty();
    }
}

// ── Drawing ─────────────────────────────────────────────────────────────────

impl WidgetApp {
    pub fn draw_notch(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, rect: Rect, theme: &Theme) {
        let resp = ui.interact(rect, egui::Id::new("notch"), Sense::click_and_drag());
        self.handle_drag(ctx, &resp);
        if resp.clicked() {
            self.expanded = true;
            self.focus_editor = matches!(self.view, View::Edit(_));
        }

        if (self.store.edge.notch_size() - self.notch).length() > 2.0 {
            return;
        }

        let painter = ui.painter();
        let open_tasks: usize = self.store.notes.iter().map(|n| { let (d, t) = progress(&n.text); t - d }).sum();
        let hov = resp.hovered();
        let label_col = if hov { theme.red } else { theme.text };

        if self.store.edge.vertical() {
            let cx = rect.center().x;
            painter.circle_filled(Pos2::new(cx, rect.top() + 14.0), 3.5, theme.red);
            painter.text(Pos2::new(cx, rect.top() + 27.0), Align2::CENTER_TOP, "N\nO\nT\nE\nS", doto(11.5), label_col);
            // Open tasks as a column of dots.
            if open_tasks > 0 {
                for k in 0..open_tasks.min(3) {
                    painter.circle_filled(Pos2::new(cx, rect.bottom() - 14.0 - k as f32 * 6.0), 1.6, theme.muted);
                }
            } else {
                painter.circle_filled(Pos2::new(cx, rect.bottom() - 14.0), 1.8, theme.border);
            }
        } else {
            let cy = rect.center().y;
            painter.circle_filled(Pos2::new(rect.left() + 16.0, cy), 4.0, theme.red);
            painter.text(Pos2::new(rect.left() + 28.0, cy + 1.0), Align2::LEFT_CENTER, "NOTES", doto(17.0), label_col);
            painter.text(
                Pos2::new(rect.right() - 14.0, cy + 1.0),
                Align2::RIGHT_CENTER,
                format!("{:02}", self.store.notes.len()),
                doto(15.0),
                theme.muted,
            );
        }
    }

    pub fn draw_panel(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, rect: Rect, theme: &Theme) {
        let painter = ui.painter().clone();

        let header_h = 72.0;
        let header = Rect::from_min_max(rect.left_top(), Pos2::new(rect.right(), rect.top() + header_h));
        dot_grid(&painter, header.shrink2(Vec2::new(1.0, 1.0)), 9.0, theme.dots);

        let header_resp = ui.interact(header, egui::Id::new("header_drag"), Sense::drag());
        self.handle_drag(ctx, &header_resp);

        // Wordmark: "NOTES" in Doto with the signature red dot.
        let title_rect = painter.text(Pos2::new(rect.left() + PAD, rect.top() + 30.0), Align2::LEFT_CENTER, "NOTES", doto(32.0), theme.text);
        painter.circle_filled(Pos2::new(title_rect.right() + 7.0, title_rect.top() + 8.0), 3.5, theme.red);
        painter.text(
            Pos2::new(rect.left() + PAD + 1.0, rect.top() + 56.0),
            Align2::LEFT_CENTER,
            Local::now().format("%a · %d %b").to_string().to_uppercase(),
            mono(10.5),
            theme.muted,
        );

        let by = rect.top() + 30.0;
        let close_c = Pos2::new(rect.right() - PAD - 6.0, by);
        let theme_c = close_c - Vec2::new(30.0, 0.0);
        let auto_c = theme_c - Vec2::new(30.0, 0.0);
        let list_c = auto_c - Vec2::new(30.0, 0.0);
        let add_c = list_c - Vec2::new(30.0, 0.0);

        if title_button(ui, add_c, "+", theme.text, "btn_add", theme).on_hover_text("New note  (Ctrl+N)").clicked() {
            self.new_note();
        }
        if title_button(ui, list_c, "☑", theme.text, "btn_list", theme).on_hover_text("New to-do list  (Ctrl+T)").clicked() {
            self.new_list();
        }
        let auto_col = if self.store.autostart { theme.red } else { theme.muted };
        if title_button(ui, auto_c, "⚡", auto_col, "btn_auto", theme).on_hover_text("Launch at startup").clicked() {
            self.store.autostart = !self.store.autostart;
            set_autostart_registry(self.store.autostart);
            self.mark_dirty();
        }
        let theme_icon = if self.store.dark_mode { "W" } else { "B" };
        if title_button(ui, theme_c, theme_icon, theme.text, "btn_theme", theme).on_hover_text("Toggle theme").clicked() {
            self.store.dark_mode = !self.store.dark_mode;
            self.mark_dirty();
        }
        if title_button(ui, close_c, "×", theme.text, "btn_close", theme).on_hover_text("Quit").clicked() {
            self.cleanup_empty();
            self.save_now();
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }

        // Hairline under the header.
        painter.hline(rect.x_range().shrink(PAD), header.bottom(), Stroke::new(1.0, theme.border));

        let body_top = header.bottom() + 10.0;
        match self.view {
            View::List => self.draw_list(ui, ctx, rect, body_top, theme),
            View::Edit(id) => self.draw_editor(ui, rect, body_top, id, theme),
        }
    }

    fn draw_footer(&self, painter: &egui::Painter, rect: Rect, left: String, theme: &Theme) {
        let y = rect.bottom() - 20.0;
        painter.text(Pos2::new(rect.left() + PAD, y), Align2::LEFT_CENTER, left, mono(10.5), theme.muted);
        painter.text(Pos2::new(rect.right() - PAD, y), Align2::RIGHT_CENTER, self.hotkey_label, mono(10.5), theme.muted);
    }

    pub fn draw_list(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, rect: Rect, y: f32, theme: &Theme) {
        let painter = ui.painter().clone();

        // Search pill.
        let search_rect = Rect::from_min_max(Pos2::new(rect.left() + PAD, y), Pos2::new(rect.right() - PAD, y + 34.0));
        let pill = Rounding::same(17.0);
        painter.rect_filled(search_rect, pill, theme.hover);
        painter.rect_stroke(search_rect, pill, Stroke::new(1.0, theme.border));
        painter.circle_stroke(Pos2::new(search_rect.left() + 18.0, search_rect.center().y - 1.0), 4.5, Stroke::new(1.3, theme.muted));
        painter.line_segment(
            [
                Pos2::new(search_rect.left() + 21.5, search_rect.center().y + 2.5),
                Pos2::new(search_rect.left() + 24.5, search_rect.center().y + 5.5),
            ],
            Stroke::new(1.3, theme.muted),
        );
        let inner_search = Rect::from_min_max(
            Pos2::new(search_rect.left() + 34.0, search_rect.top() + 8.0),
            Pos2::new(search_rect.right() - 14.0, search_rect.bottom() - 8.0),
        );
        ui.allocate_ui_at_rect(inner_search, |ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.search)
                    .frame(false)
                    .desired_width(f32::INFINITY)
                    .font(FontId::proportional(14.0))
                    .text_color(theme.text)
                    .hint_text(RichText::new("Search").font(FontId::proportional(14.0)).color(theme.muted)),
            );
        });

        let list_rect = Rect::from_min_max(Pos2::new(rect.left() + PAD - 6.0, y + 46.0), Pos2::new(rect.right() - PAD + 6.0, rect.bottom() - 40.0));

        let q = self.search.trim().to_lowercase();
        let mut idx: Vec<usize> = (0..self.store.notes.len())
            .filter(|&i| {
                if q.is_empty() { return true; }
                let note = &self.store.notes[i];
                crate::utils::contains_ignore_case(&note.text, &q) || crate::utils::contains_ignore_case(&note.title, &q)
            })
            .collect();
        idx.sort_by_key(|&i| std::cmp::Reverse(self.store.notes[i].created));

        // Pinned notes first, then grouped by day.
        let pinned: Vec<usize> = idx.iter().copied().filter(|&i| self.store.notes[i].pinned).collect();
        let mut groups: Vec<(String, Vec<usize>)> = Vec::new();
        if !pinned.is_empty() {
            groups.push(("Pinned".to_owned(), pinned));
        }
        let today = Local::now().date_naive();
        let mut last_date: Option<NaiveDate> = None;
        for i in idx.into_iter().filter(|&i| !self.store.notes[i].pinned) {
            let date = local_dt(self.store.notes[i].created).date_naive();
            if last_date == Some(date) {
                groups.last_mut().unwrap().1.push(i);
            } else {
                groups.push((day_label(date, today), vec![i]));
                last_date = Some(date);
            }
        }

        let mut op: Option<NoteOp> = None;

        if groups.is_empty() {
            let (title, hint) = if self.store.notes.is_empty() {
                ("EMPTY", format!("+ for a note  ·  ☑ for a list  ·  {}", self.hotkey_label))
            } else {
                ("NO MATCH", "Nothing matches your search".to_owned())
            };
            let c = list_rect.center();
            dot_grid(&painter, Rect::from_center_size(c - Vec2::new(0.0, 6.0), Vec2::new(120.0, 54.0)), 9.0, theme.dots);
            painter.text(Pos2::new(c.x, c.y - 8.0), Align2::CENTER_CENTER, title, doto(30.0), theme.text);
            painter.text(Pos2::new(c.x, c.y + 26.0), Align2::CENTER_CENTER, hint, mono(10.5), theme.muted);
            let bg = ui.interact(list_rect, egui::Id::new("list_bg_empty"), Sense::click());
            bg.context_menu(|ui| list_bg_menu(ui, &mut op));
        } else {
            ui.allocate_ui_at_rect(list_rect, |ui| {
                egui::ScrollArea::vertical()
                    .id_source("note_list")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 2.0;
                        for (gi, (label, items)) in groups.iter().enumerate() {
                            if gi > 0 {
                                ui.add_space(10.0);
                            }
                            section_header(ui, label, items.len(), theme);
                            for &i in items {
                                let note = &self.store.notes[i];
                                let resp = note_row(ui, note, theme);
                                if resp.hovered() {
                                    ctx.set_cursor_icon(CursorIcon::PointingHand);
                                }
                                if resp.clicked() {
                                    op = Some(NoteOp::Open(note.id));
                                }
                                let (id, pinned, todo) = (note.id, note.pinned, note.is_todo());
                                resp.context_menu(|ui| note_menu(ui, id, pinned, todo, &mut op));
                            }
                        }
                        let rest = ui.available_rect_before_wrap();
                        if rest.height() > 0.0 {
                            let bg = ui.interact(rest, egui::Id::new("list_bg"), Sense::click());
                            bg.context_menu(|ui| list_bg_menu(ui, &mut op));
                        }
                    });
            });
        }
        if let Some(op) = op {
            self.apply_note_op(op, ctx);
        }

        let open_tasks: usize = self.store.notes.iter().map(|n| { let (d, t) = progress(&n.text); t - d }).sum();
        let footer = format!("{:02} NOTES  ·  {:02} OPEN TASKS", self.store.notes.len(), open_tasks);
        self.draw_footer(&painter, rect, footer, theme);
    }

    pub fn draw_editor(&mut self, ui: &mut egui::Ui, rect: Rect, y: f32, id: u64, theme: &Theme) {
        let Some(i) = self.store.notes.iter().position(|n| n.id == id) else {
            self.view = View::List;
            return;
        };
        let painter = ui.painter().clone();
        let ctx = ui.ctx().clone();
        let todo = self.store.notes[i].is_todo();

        // ── Toolbar ──
        let ty = y + 12.0;
        if zen_button(ui, Pos2::new(rect.left() + PAD, ty), Align2::LEFT_CENTER, "← BACK", theme.text, "btn_back", theme).clicked() {
            self.leave_editor();
            return;
        }
        let del_label = if self.confirm_delete { "CONFIRM?" } else { "DELETE" };
        let del_col = if self.confirm_delete { theme.red } else { theme.muted };
        let del = zen_button(ui, Pos2::new(rect.right() - PAD, ty), Align2::RIGHT_CENTER, del_label, del_col, "btn_del", theme);
        if del.clicked() {
            if self.confirm_delete {
                self.store.notes.remove(i);
                self.view = View::List;
                self.confirm_delete = false;
                self.mark_dirty();
                return;
            }
            self.confirm_delete = true;
        }
        let mode_label = if todo { "→ TEXT" } else { "→ LIST" };
        let mode_x = del.rect.left() - 8.0;
        if zen_button(ui, Pos2::new(mode_x, ty), Align2::RIGHT_CENTER, mode_label, theme.text, "btn_mode", theme)
            .on_hover_text(if todo { "Convert to plain note" } else { "Convert to to-do list" })
            .clicked()
        {
            let note = &mut self.store.notes[i];
            if todo {
                note.text = list_to_text(&note.text);
                self.focus_editor = true;
                self.open_todo = None;
            } else {
                note.text = text_to_list(&note.text);
                self.todo_focus = Some(note.text.lines().count().saturating_sub(1));
                self.open_todo = Some(crate::todo::parse_items(&note.text));
            }
            note.updated = Local::now().timestamp();
            self.mark_dirty();
            return;
        }

        // ── Heading ──
        let hy = y + 52.0;
        let mut title_text = if self.store.notes[i].title.is_empty() {
            if todo { "TO-DO".to_string() } else { "".to_string() }
        } else {
            self.store.notes[i].title.clone()
        };

        if todo || !title_text.is_empty() || self.focus_editor {
            let stamp_w = 115.0;
            let title_w = (rect.width() - PAD * 2.0 - stamp_w).max(80.0);
            let title_rect = Rect::from_min_size(
                Pos2::new(rect.left() + PAD, hy - 14.0),
                Vec2::new(title_w, 28.0),
            );
            
            // Dynamic font sizing: scales smoothly as title grows so it fits appropriately.
            let char_count = title_text.chars().count();
            let font_size = match char_count {
                0..=8 => 24.0,
                9..=14 => 20.0,
                15..=20 => 16.5,
                21..=28 => 14.0,
                _ => 12.0,
            };

            let mut layouter = |ui: &egui::Ui, string: &str, wrap_width: f32| {
                let layout_job = egui::text::LayoutJob::simple(
                    string.to_owned(),
                    doto(font_size),
                    theme.text,
                    wrap_width,
                );
                ui.fonts(|f| f.layout_job(layout_job))
            };

            let title_changed = ui.put(
                title_rect,
                egui::TextEdit::singleline(&mut title_text)
                    .frame(false)
                    .clip_text(true)
                    .layouter(&mut layouter)
                    .hint_text(if todo { "TO-DO" } else { "TITLE" })
            ).changed();

            if title_changed {
                self.store.notes[i].title = title_text;
                self.mark_dirty();
            }
        }

        let stamp = local_dt(self.store.notes[i].created).format("%d.%m.%y  %H:%M").to_string();
        painter.text(Pos2::new(rect.right() - PAD, hy), Align2::RIGHT_CENTER, stamp, mono(10.5), theme.muted);

        let mut top = hy + 22.0;
        if todo {
            let (done, total) = progress(&self.store.notes[i].text);
            let width = rect.width() - PAD * 2.0 - 60.0;
            let count = ((width / 7.0) as usize).max(1);
            let filled = if total == 0 { 0 } else { done * count / total };
            dot_progress(&painter, Pos2::new(rect.left() + PAD + 2.0, top), count, filled, 7.0, theme);
            painter.text(Pos2::new(rect.right() - PAD, top), Align2::RIGHT_CENTER, format!("{done}/{total}"), mono(11.0), theme.text);
            top += 14.0;
        }

        let area = Rect::from_min_max(Pos2::new(rect.left() + PAD - 6.0, top), Pos2::new(rect.right() - PAD + 6.0, rect.bottom() - 40.0));

        let changed = if todo {
            if self.focus_editor {
                self.focus_editor = false;
                if self.todo_focus.is_none() {
                    let items = parse_items(&self.store.notes[i].text);
                    let target = items.iter().position(|it| it.done.is_some() && it.text.is_empty()).unwrap_or(items.len() - 1);
                    self.todo_focus = Some(target);
                }
            }
            self.draw_todo(ui, area, i, theme)
        } else {
            self.draw_text_editor(ui, area, i, theme)
        };

        if changed {
            if let Some(note) = self.store.notes.get_mut(i) {
                note.updated = Local::now().timestamp();
            }
            self.confirm_delete = false;
            self.mark_dirty();
        }

        let Some(note) = self.store.notes.get(i) else { return };
        let footer = if todo {
            let (d, t) = progress(&note.text);
            format!("{:02} TASKS  ·  {:02} LEFT", t, t - d)
        } else {
            format!("{:02} WORDS", note.text.split_whitespace().count())
        };
        self.draw_footer(&painter, rect, footer, theme);
        let _ = ctx;
    }

    fn draw_text_editor(&mut self, ui: &mut egui::Ui, area: Rect, i: usize, theme: &Theme) -> bool {
        let note_id = self.store.notes[i].id;
        let images: Vec<String> = self.store.notes[i]
            .text
            .lines()
            .filter_map(|l| {
                let l = l.trim();
                (l.starts_with("![") && l.ends_with(')')).then(|| l.find("](").map(|s| l[s + 2..l.len() - 1].to_owned()))?
            })
            .collect();

        let mut changed = false;
        let mut to_list = false;
        let mut copy = false;
        let mut pin = false;
        let pinned = self.store.notes[i].pinned;

        ui.allocate_ui_at_rect(area.shrink2(Vec2::new(6.0, 0.0)), |ui| {
            egui::ScrollArea::vertical()
                .id_source(("note_editor", note_id))
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for path in &images {
                        ui.add(
                            egui::Image::new(format!("file://{path}"))
                                .max_width(ui.available_width())
                                .max_height(180.0)
                                .rounding(Rounding::same(10.0)),
                        );
                        ui.add_space(6.0);
                    }
                    let text_id = egui::Id::new(("editor_text", note_id));
                    let resp = ui.add(
                        egui::TextEdit::multiline(&mut self.store.notes[i].text)
                            .id(text_id)
                            .frame(false)
                            .desired_width(f32::INFINITY)
                            .desired_rows(16)
                            .font(FontId::proportional(15.0))
                            .text_color(theme.text)
                            .hint_text(RichText::new("Start typing…").font(FontId::proportional(15.0)).color(theme.muted)),
                    );
                    // Grab focus once when the note opens — never every frame.
                    if self.focus_editor {
                        resp.request_focus();
                        self.focus_editor = false;
                    }
                    changed = resp.changed();
                    resp.context_menu(|ui| {
                        ui.set_min_width(190.0);
                        ui.label(RichText::new("NOTE").font(mono(10.5)).weak());
                        if ui.add(egui::Button::new("Convert to to-do list").min_size(Vec2::new(190.0, 26.0))).clicked() {
                            to_list = true;
                            ui.close_menu();
                        }
                        if ui.add(egui::Button::new("Copy note").min_size(Vec2::new(190.0, 26.0))).clicked() {
                            copy = true;
                            ui.close_menu();
                        }
                        if ui.add(egui::Button::new(if pinned { "Unpin" } else { "Pin to top" }).min_size(Vec2::new(190.0, 26.0))).clicked() {
                            pin = true;
                            ui.close_menu();
                        }
                    });
                });
        });

        if to_list {
            let note = &mut self.store.notes[i];
            note.text = text_to_list(&note.text);
            self.todo_focus = Some(note.text.lines().count().saturating_sub(1));
            changed = true;
        }
        if copy {
            let text = self.store.notes[i].text.clone();
            let ctx = ui.ctx().clone();
            self.copy_text(&ctx, text);
        }
        if pin {
            self.store.notes[i].pinned = !pinned;
            changed = true;
        }
        changed
    }
}

// ── List context menus ──────────────────────────────────────────────────────

fn menu_btn(ui: &mut egui::Ui, label: &str, color: Option<Color32>) -> bool {
    let text = match color {
        Some(c) => RichText::new(label).size(13.5).color(c),
        None => RichText::new(label).size(13.5),
    };
    let clicked = ui.add(egui::Button::new(text).min_size(Vec2::new(190.0, 26.0))).clicked();
    if clicked {
        ui.close_menu();
    }
    clicked
}

fn note_menu(ui: &mut egui::Ui, id: u64, pinned: bool, todo: bool, op: &mut Option<NoteOp>) {
    ui.set_min_width(190.0);
    ui.label(RichText::new("NOTE").font(mono(10.5)).weak());
    if menu_btn(ui, "Open", None) {
        *op = Some(NoteOp::Open(id));
    }
    if menu_btn(ui, if pinned { "Unpin" } else { "Pin to top" }, None) {
        *op = Some(NoteOp::Pin(id));
    }
    if menu_btn(ui, "Duplicate", None) {
        *op = Some(NoteOp::Duplicate(id));
    }
    if menu_btn(ui, "Copy contents", None) {
        *op = Some(NoteOp::Copy(id));
    }
    if todo {
        if menu_btn(ui, "Convert to plain note", None) {
            *op = Some(NoteOp::ToText(id));
        }
    } else if menu_btn(ui, "Convert to to-do list", None) {
        *op = Some(NoteOp::ToList(id));
    }
    ui.separator();
    if menu_btn(ui, "Delete", Some(Color32::from_rgb(235, 45, 45))) {
        *op = Some(NoteOp::Delete(id));
    }
}

fn list_bg_menu(ui: &mut egui::Ui, op: &mut Option<NoteOp>) {
    ui.set_min_width(190.0);
    ui.label(RichText::new("NOTES").font(mono(10.5)).weak());
    if menu_btn(ui, "New note", None) {
        *op = Some(NoteOp::NewNote);
    }
    if menu_btn(ui, "New to-do list", None) {
        *op = Some(NoteOp::NewList);
    }
    ui.separator();
    if menu_btn(ui, "Clear clipboard notes", None) {
        *op = Some(NoteOp::ClearClipboard);
    }
}
