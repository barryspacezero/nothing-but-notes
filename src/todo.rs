use eframe::egui::{self, Align2, CursorIcon, FontId, Key, Pos2, Rect, Rounding, Sense, Stroke, Vec2};

use crate::*;

// ── Checklist model ─────────────────────────────────────────────────────────
//
// To-do lists are stored as plain markdown inside `Note::text`, so they stay
// readable in notes.json and convert losslessly to/from plain text:
//
//   Groceries          <- plain line, rendered as a Doto sub-heading
//   - [ ] milk         <- open task
//   - [x] eggs         <- completed task

/// Split a line into `(Some(done) | None, content)`.
pub fn parse_line(line: &str) -> (Option<bool>, &str) {
    let t = line.trim_start();
    for (prefix, done) in [("- [ ]", false), ("- [x]", true), ("- [X]", true)] {
        if let Some(rest) = t.strip_prefix(prefix) {
            return (Some(done), rest.strip_prefix(' ').unwrap_or(rest));
        }
    }
    (None, line)
}

pub fn is_todo(text: &str) -> bool {
    text.lines().any(|l| parse_line(l).0.is_some())
}

/// `(completed, total)` task counts.
pub fn progress(text: &str) -> (usize, usize) {
    text.lines().fold((0, 0), |(d, t), l| match parse_line(l).0 {
        Some(true) => (d + 1, t + 1),
        Some(false) => (d, t + 1),
        None => (d, t),
    })
}

fn is_image(line: &str) -> bool {
    let l = line.trim();
    l.starts_with("![") && l.contains("](") && l.ends_with(')')
}

/// Plain text → checklist. Every non-empty line becomes an open task.
pub fn text_to_list(text: &str) -> String {
    let lines: Vec<String> = text
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| match parse_line(l) {
            (Some(_), _) => l.to_owned(),
            (None, _) if is_image(l) => l.to_owned(),
            (None, c) => format!("- [ ] {}", c.trim()),
        })
        .collect();
    if lines.is_empty() { "- [ ] ".to_owned() } else { lines.join("\n") }
}

/// Checklist → plain text (markers removed).
pub fn list_to_text(text: &str) -> String {
    text.lines().map(|l| parse_line(l).1).collect::<Vec<_>>().join("\n")
}

#[derive(Clone)]
pub struct Item {
    /// `None` = plain line (heading), `Some(done)` = task.
    pub done: Option<bool>,
    pub text: String,
}

impl Item {
    pub fn task(text: &str) -> Self {
        Self { done: Some(false), text: text.to_owned() }
    }
}

pub fn parse_items(text: &str) -> Vec<Item> {
    let mut items: Vec<Item> = text
        .lines()
        .map(|l| {
            let (done, content) = parse_line(l);
            Item { done, text: content.to_owned() }
        })
        .collect();
    if items.is_empty() {
        items.push(Item::task(""));
    }
    items
}

pub fn serialize_items(items: &[Item]) -> String {
    items
        .iter()
        .map(|it| match it.done {
            Some(true) => format!("- [x] {}", it.text),
            Some(false) => format!("- [ ] {}", it.text),
            None => it.text.clone(),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

// ── Operations ──────────────────────────────────────────────────────────────

#[derive(Clone, Copy)]
pub enum ItemOp {
    Toggle(usize),
    InsertAbove(usize),
    InsertBelow(usize),
    MoveUp(usize),
    MoveDown(usize),
    Duplicate(usize),
    MakeHeading(usize),
    MakeTask(usize),
    Copy(usize),
    Delete(usize),
    /// Backspace on an empty row: delete and jump to the previous one.
    Merge(usize),
}

#[derive(Clone, Copy)]
pub enum ListOp {
    Add,
    CheckAll,
    UncheckAll,
    ClearDone,
    SortDone,
    Reverse,
    CopyAll,
    ConvertToText,
}

/// Stable-sort each run of consecutive tasks so completed ones sink,
/// without moving tasks across headings.
fn sort_done(items: &mut [Item]) {
    let mut start = 0;
    while start < items.len() {
        if items[start].done.is_none() {
            start += 1;
            continue;
        }
        let mut end = start;
        while end < items.len() && items[end].done.is_some() {
            end += 1;
        }
        items[start..end].sort_by_key(|it| it.done == Some(true));
        start = end;
    }
}

// ── Checklist editor ────────────────────────────────────────────────────────

const ROW_H: f32 = 36.0;

impl WidgetApp {
    /// Interactive checklist editor. Returns `true` if the note changed.
    pub fn draw_todo(&mut self, ui: &mut egui::Ui, area: Rect, i: usize, theme: &Theme) -> bool {
        let note_id = self.store.notes[i].id;
        let mut items = self.open_todo.take().unwrap_or_else(|| parse_items(&self.store.notes[i].text));
        let mut changed = false;
        let mut item_op: Option<ItemOp> = None;
        let mut list_op: Option<ListOp> = None;
        let focus_req = self.todo_focus.take();
        let ctx = ui.ctx().clone();

        let (key_enter, key_back, key_up, key_down, alt, ctrl) = ctx.input(|inp| {
            (
                inp.key_pressed(Key::Enter),
                inp.key_pressed(Key::Backspace),
                inp.key_pressed(Key::ArrowUp),
                inp.key_pressed(Key::ArrowDown),
                inp.modifiers.alt,
                inp.modifiers.ctrl,
            )
        });

        ui.allocate_ui_at_rect(area, |ui| {
            egui::ScrollArea::vertical()
                .id_source(("todo_scroll", note_id))
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    let n = items.len();
                    for idx in 0..n {
                        let kind = items[idx].done;
                        let heading = kind.is_none();
                        let done = kind == Some(true);
                        let h = if heading { ROW_H + 6.0 } else { ROW_H };
                        let (row, row_resp) =
                            ui.allocate_exact_size(Vec2::new(ui.available_width(), h), Sense::click());
                        let hovered = ui.rect_contains_pointer(row);
                        let text_id = egui::Id::new(("todo_item", note_id, idx));
                        let focused = ctx.memory(|m| m.has_focus(text_id));

                        let p = ui.painter();
                        if hovered || focused {
                            p.rect_filled(row, Rounding::same(8.0), theme.hover);
                        }

                        // Dot checkbox.
                        let mut text_left = row.left() + 10.0;
                        if !heading {
                            let c = Pos2::new(row.left() + 18.0, row.center().y);
                            let hit = Rect::from_center_size(c, Vec2::splat(26.0));
                            let chk = ui.interact(hit, egui::Id::new(("todo_chk", note_id, idx)), Sense::click());
                            if chk.hovered() {
                                ctx.set_cursor_icon(CursorIcon::PointingHand);
                            }
                            if chk.clicked() {
                                item_op = Some(ItemOp::Toggle(idx));
                            }
                            let ring = if chk.hovered() { theme.red } else { theme.text };
                            let p = ui.painter();
                            if done {
                                p.circle_filled(c, 8.0, theme.red);
                                p.circle_filled(c, 2.5, theme.bg);
                            } else {
                                p.circle_stroke(c, 7.5, Stroke::new(1.5_f32, ring));
                            }
                            chk.context_menu(|ui| item_menu(ui, idx, n, kind, &mut item_op, &mut list_op));
                            text_left = row.left() + 36.0;
                        }

                        // Inline text field.
                        let color = if done { theme.muted } else { theme.text };
                        let font = if heading { doto(20.0) } else { FontId::proportional(15.0) };
                        let strike = if done { Stroke::new(1.2_f32, theme.muted) } else { Stroke::NONE };
                        let mut layouter = |ui: &egui::Ui, s: &str, wrap: f32| {
                            let mut job = egui::text::LayoutJob::single_section(
                                s.to_owned(),
                                egui::TextFormat { font_id: font.clone(), color, strikethrough: strike, ..Default::default() },
                            );
                            job.wrap.max_width = wrap;
                            ui.fonts(|f| f.layout_job(job))
                        };
                        let was_empty = items[idx].text.is_empty();
                        let hint = if heading { "HEADING" } else { "New task" };
                        let text_rect = Rect::from_min_max(
                            Pos2::new(text_left, row.top() + 4.0),
                            Pos2::new(row.right() - 8.0, row.bottom() - 4.0),
                        );
                        let resp = ui.put(
                            text_rect,
                            egui::TextEdit::singleline(&mut items[idx].text)
                                .id(text_id)
                                .frame(false)
                                .clip_text(true)
                                .desired_width(f32::INFINITY)
                                .vertical_align(egui::Align::Center)
                                .hint_text(egui::RichText::new(hint).color(theme.muted.linear_multiply(0.6)))
                                .layouter(&mut layouter),
                        );
                        if resp.changed() {
                            changed = true;
                        }
                        if focus_req == Some(idx) {
                            resp.request_focus();
                        }

                        // Keyboard flow: Enter = new task, Backspace on empty = delete,
                        // ↑/↓ = navigate, Alt+↑/↓ = reorder, Ctrl+Enter = toggle.
                        if resp.lost_focus() && key_enter {
                            item_op = Some(if ctrl { ItemOp::Toggle(idx) } else { ItemOp::InsertBelow(idx) });
                            if ctrl {
                                self.todo_focus = Some(idx);
                            }
                        } else if resp.has_focus() {
                            if key_back && was_empty && n > 1 {
                                item_op = Some(ItemOp::Merge(idx));
                            } else if key_up {
                                if alt {
                                    item_op = Some(ItemOp::MoveUp(idx));
                                } else if idx > 0 {
                                    self.todo_focus = Some(idx - 1);
                                }
                            } else if key_down {
                                if alt {
                                    item_op = Some(ItemOp::MoveDown(idx));
                                } else if idx + 1 < n {
                                    self.todo_focus = Some(idx + 1);
                                }
                            }
                        }

                        resp.context_menu(|ui| item_menu(ui, idx, n, kind, &mut item_op, &mut list_op));
                        row_resp.context_menu(|ui| item_menu(ui, idx, n, kind, &mut item_op, &mut list_op));
                    }

                    // "+ Add task" row.
                    ui.add_space(4.0);
                    let (add_row, add_resp) =
                        ui.allocate_exact_size(Vec2::new(ui.available_width(), ROW_H), Sense::click());
                    let col = if add_resp.hovered() { theme.text } else { theme.muted };
                    if add_resp.hovered() {
                        ctx.set_cursor_icon(CursorIcon::PointingHand);
                        ui.painter().rect_filled(add_row, Rounding::same(8.0), theme.hover);
                    }
                    let c = Pos2::new(add_row.left() + 18.0, add_row.center().y);
                    ui.painter().text(c, Align2::CENTER_CENTER, "+", FontId::proportional(18.0), col);
                    ui.painter().text(
                        Pos2::new(add_row.left() + 36.0, add_row.center().y),
                        Align2::LEFT_CENTER,
                        "ADD TASK",
                        mono(12.0),
                        col,
                    );
                    if add_resp.clicked() {
                        list_op = Some(ListOp::Add);
                    }
                    add_resp.context_menu(|ui| list_menu(ui, &mut list_op));

                    // Remaining empty space: right-click for list operations.
                    let rest = ui.available_rect_before_wrap();
                    if rest.height() > 0.0 {
                        let bg = ui.interact(rest, egui::Id::new(("todo_bg", note_id)), Sense::click());
                        if bg.clicked() {
                            list_op = Some(ListOp::Add);
                        }
                        bg.context_menu(|ui| list_menu(ui, &mut list_op));
                    }
                });
        });

        if let Some(op) = item_op {
            changed |= self.apply_item_op(&mut items, op, &ctx);
        }
        if let Some(op) = list_op {
            match op {
                ListOp::ConvertToText => {
                    self.store.notes[i].text = list_to_text(&serialize_items(&items));
                    self.focus_editor = true;
                    return true;
                }
                op => changed |= self.apply_list_op(&mut items, op, &ctx),
            }
        }

        if changed {
            self.store.notes[i].text = serialize_items(&items);
            ctx.request_repaint();
        }
        if self.todo_focus.is_some() {
            ctx.request_repaint();
        }
        self.open_todo = Some(items);
        changed
    }

    fn apply_item_op(&mut self, items: &mut Vec<Item>, op: ItemOp, ctx: &egui::Context) -> bool {
        let n = items.len();
        match op {
            ItemOp::Toggle(i) => {
                if let Some(d) = &mut items[i].done {
                    *d = !*d;
                }
            }
            ItemOp::InsertAbove(i) => {
                items.insert(i, Item::task(""));
                self.todo_focus = Some(i);
            }
            ItemOp::InsertBelow(i) => {
                items.insert(i + 1, Item::task(""));
                self.todo_focus = Some(i + 1);
            }
            ItemOp::MoveUp(i) if i > 0 => {
                items.swap(i, i - 1);
                self.todo_focus = Some(i - 1);
            }
            ItemOp::MoveDown(i) if i + 1 < n => {
                items.swap(i, i + 1);
                self.todo_focus = Some(i + 1);
            }
            ItemOp::Duplicate(i) => {
                let copy = items[i].clone();
                items.insert(i + 1, copy);
            }
            ItemOp::MakeHeading(i) => items[i].done = None,
            ItemOp::MakeTask(i) => items[i].done = Some(false),
            ItemOp::Copy(i) => {
                self.copy_text(ctx, items[i].text.clone());
                return false;
            }
            ItemOp::Delete(i) | ItemOp::Merge(i) => {
                items.remove(i);
                if items.is_empty() {
                    items.push(Item::task(""));
                }
                if matches!(op, ItemOp::Merge(_)) {
                    self.todo_focus = Some(i.saturating_sub(1));
                }
            }
            _ => return false,
        }
        true
    }

    fn apply_list_op(&mut self, items: &mut Vec<Item>, op: ListOp, ctx: &egui::Context) -> bool {
        match op {
            ListOp::Add => {
                // Reuse a trailing empty task instead of stacking blanks.
                let last_empty = items.last().is_some_and(|it| it.done.is_some() && it.text.is_empty());
                if !last_empty {
                    items.push(Item::task(""));
                }
                self.todo_focus = Some(items.len() - 1);
            }
            ListOp::CheckAll => items.iter_mut().filter_map(|it| it.done.as_mut()).for_each(|d| *d = true),
            ListOp::UncheckAll => items.iter_mut().filter_map(|it| it.done.as_mut()).for_each(|d| *d = false),
            ListOp::ClearDone => {
                items.retain(|it| it.done != Some(true));
                if items.is_empty() {
                    items.push(Item::task(""));
                }
            }
            ListOp::SortDone => sort_done(items),
            ListOp::Reverse => items.reverse(),
            ListOp::CopyAll => {
                let text = items
                    .iter()
                    .map(|it| match it.done {
                        Some(true) => format!("☑ {}", it.text),
                        Some(false) => format!("☐ {}", it.text),
                        None => it.text.to_uppercase(),
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                self.copy_text(ctx, text);
                return false;
            }
            ListOp::ConvertToText => return false,
        }
        true
    }
}

// ── Context menus ───────────────────────────────────────────────────────────

fn menu_item(ui: &mut egui::Ui, label: &str, shortcut: &str) -> bool {
    let text = if shortcut.is_empty() {
        egui::RichText::new(label).size(13.5)
    } else {
        egui::RichText::new(format!("{label:<18}{shortcut}")).size(13.5)
    };
    let clicked = ui.add(egui::Button::new(text).min_size(Vec2::new(190.0, 26.0))).clicked();
    if clicked {
        ui.close_menu();
    }
    clicked
}

fn menu_title(ui: &mut egui::Ui, title: &str) {
    ui.add_space(2.0);
    ui.label(egui::RichText::new(title).font(mono(10.5)).weak());
    ui.add_space(2.0);
}

fn item_menu(
    ui: &mut egui::Ui,
    idx: usize,
    n: usize,
    done: Option<bool>,
    item_op: &mut Option<ItemOp>,
    list_op: &mut Option<ListOp>,
) {
    ui.set_min_width(190.0);
    menu_title(ui, "ITEM");
    match done {
        Some(d) => {
            if menu_item(ui, if d { "Mark as not done" } else { "Mark as done" }, "") {
                *item_op = Some(ItemOp::Toggle(idx));
            }
        }
        None => {}
    }
    if menu_item(ui, "Insert above", "") {
        *item_op = Some(ItemOp::InsertAbove(idx));
    }
    if menu_item(ui, "Insert below", "") {
        *item_op = Some(ItemOp::InsertBelow(idx));
    }
    if idx > 0 && menu_item(ui, "Move up", "") {
        *item_op = Some(ItemOp::MoveUp(idx));
    }
    if idx + 1 < n && menu_item(ui, "Move down", "") {
        *item_op = Some(ItemOp::MoveDown(idx));
    }
    if menu_item(ui, "Duplicate", "") {
        *item_op = Some(ItemOp::Duplicate(idx));
    }
    if done.is_some() {
        if menu_item(ui, "Turn into heading", "") {
            *item_op = Some(ItemOp::MakeHeading(idx));
        }
    } else if menu_item(ui, "Turn into task", "") {
        *item_op = Some(ItemOp::MakeTask(idx));
    }
    if menu_item(ui, "Copy text", "") {
        *item_op = Some(ItemOp::Copy(idx));
    }
    if menu_item(ui, "Delete", "") {
        *item_op = Some(ItemOp::Delete(idx));
    }
    ui.separator();
    ui.menu_button("List  ›", |ui| list_menu(ui, list_op));
}

fn list_menu(ui: &mut egui::Ui, list_op: &mut Option<ListOp>) {
    ui.set_min_width(190.0);
    menu_title(ui, "LIST");
    let entries = [
        ("Add task", ListOp::Add),
        ("Check all", ListOp::CheckAll),
        ("Uncheck all", ListOp::UncheckAll),
        ("Clear completed", ListOp::ClearDone),
        ("Completed to bottom", ListOp::SortDone),
        ("Reverse order", ListOp::Reverse),
        ("Copy as text", ListOp::CopyAll),
        ("Convert to plain note", ListOp::ConvertToText),
    ];
    for (label, op) in entries {
        if menu_item(ui, label, "") {
            *list_op = Some(op);
        }
    }
}
