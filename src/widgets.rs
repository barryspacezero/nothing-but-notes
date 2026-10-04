use eframe::egui::{self, Align2, Color32, CursorIcon, FontId, Painter, Pos2, Rect, Response, Rounding, Sense, Stroke, Vec2};

use crate::*;

// ── Widgets ─────────────────────────────────────────────────────────────────

/// Circular icon button used in the header.
pub fn title_button(ui: &mut egui::Ui, center: Pos2, label: &str, color: Color32, id: &str, theme: &Theme) -> Response {
    let r = Rect::from_center_size(center, Vec2::splat(26.0));
    let resp = ui.interact(r, egui::Id::new(id), Sense::click());
    let p = ui.painter();
    if resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        p.circle_filled(center, 13.0, theme.hover);
        p.circle_stroke(center, 13.0, Stroke::new(1.0_f32, theme.border));
    }
    let col = if resp.is_pointer_button_down_on() { theme.red } else { color };
    p.text(center, Align2::CENTER_CENTER, label, FontId::proportional(15.0), col);
    resp
}

/// Uppercase mono pill button ("← BACK", "DELETE").
pub fn zen_button(ui: &mut egui::Ui, pos: Pos2, align: Align2, text: &str, color: Color32, id: &str, theme: &Theme) -> Response {
    let font = mono(11.5);
    let size = ui.painter().layout_no_wrap(text.to_owned(), font.clone(), color).size();
    let r = align.anchor_size(pos, size).expand2(Vec2::new(10.0, 6.0));

    let resp = ui.interact(r, egui::Id::new(id), Sense::click());
    let p = ui.painter();
    p.rect_stroke(r, Rounding::same(r.height() / 2.0), Stroke::new(1.0_f32, theme.border));
    if resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        p.rect_filled(r, Rounding::same(r.height() / 2.0), theme.hover);
    }
    let col = if resp.is_pointer_button_down_on() { theme.red } else { color };
    p.text(r.center(), Align2::CENTER_CENTER, text, font, col);
    resp
}

/// Faint dot-matrix texture, fading out towards the bottom of `rect`.
pub fn dot_grid(p: &Painter, rect: Rect, step: f32, color: Color32) {
    let mut y = rect.top() + step / 2.0;
    while y < rect.bottom() {
        let fade = 1.0 - (y - rect.top()) / rect.height();
        let c = color.linear_multiply(fade.max(0.0));
        let mut x = rect.left() + step / 2.0;
        while x < rect.right() {
            p.circle_filled(Pos2::new(x, y), 1.0, c);
            x += step;
        }
        y += step;
    }
}

/// A row of `count` dots, the first `filled` of them lit — Nothing-style progress.
pub fn dot_progress(p: &Painter, left_center: Pos2, count: usize, filled: usize, gap: f32, theme: &Theme) {
    for k in 0..count {
        let c = left_center + Vec2::new(k as f32 * gap, 0.0);
        if k < filled {
            p.circle_filled(c, 2.2, theme.red);
        } else {
            p.circle_filled(c, 2.2, theme.border);
        }
    }
}

/// "TODAY ········· 03" style group header in Doto.
pub fn section_header(ui: &mut egui::Ui, label: &str, count: usize, theme: &Theme) {
    let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 28.0), Sense::hover());
    let p = ui.painter();
    let label_rect = p.text(
        Pos2::new(r.left() + 2.0, r.center().y),
        Align2::LEFT_CENTER,
        label.to_uppercase(),
        doto(17.0),
        theme.text,
    );
    let count_txt = format!("{:02}", count);
    let count_rect = p.text(Pos2::new(r.right() - 2.0, r.center().y), Align2::RIGHT_CENTER, count_txt, mono(11.0), theme.muted);

    // Dotted leader between the label and the count.
    let mut x = label_rect.right() + 10.0;
    while x < count_rect.left() - 8.0 {
        p.circle_filled(Pos2::new(x, r.center().y + 1.0), 0.9, theme.border);
        x += 6.0;
    }
}

pub fn note_row(ui: &mut egui::Ui, note: &Note, theme: &Theme) -> Response {
    let (r, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 58.0), Sense::click());
    let hov = resp.hovered();
    let p = ui.painter();

    if hov {
        p.rect_filled(r, Rounding::same(10.0), theme.hover);
    }

    // Left marker: red dot for pinned, ring for to-do lists, clip glyph for clipboard.
    let mx = r.left() + 12.0;
    let my = r.top() + 19.0;
    let todo = note.is_todo();
    if note.pinned {
        p.circle_filled(Pos2::new(mx, my), 3.5, theme.red);
    } else if todo {
        p.circle_stroke(Pos2::new(mx, my), 4.0, Stroke::new(1.3_f32, theme.text));
    } else if note.is_clipboard {
        p.rect_stroke(Rect::from_center_size(Pos2::new(mx, my), Vec2::new(7.0, 9.0)), Rounding::same(1.5), Stroke::new(1.2_f32, theme.muted));
    } else {
        p.circle_filled(Pos2::new(mx, my), 2.0, theme.muted);
    }

    let tx = r.left() + 26.0;
    p.text(Pos2::new(tx, r.top() + 10.0), Align2::LEFT_TOP, truncate(note.title(), 30), FontId::proportional(15.0), theme.text);

    if todo {
        let (done, total) = progress(&note.text);
        let shown = total.min(14);
        let filled = if total == 0 { 0 } else { (done * shown + total / 2) / total };
        dot_progress(p, Pos2::new(tx + 2.0, r.top() + 40.0), shown, filled, 7.0, theme);
        p.text(
            Pos2::new(tx + shown as f32 * 7.0 + 8.0, r.top() + 40.0),
            Align2::LEFT_CENTER,
            format!("{done}/{total}"),
            mono(11.0),
            theme.muted,
        );
    } else {
        p.text(Pos2::new(tx, r.top() + 32.0), Align2::LEFT_TOP, truncate(note.preview(), 40), FontId::proportional(13.0), theme.muted);
    }

    p.text(
        Pos2::new(r.right() - 10.0, r.top() + 11.0),
        Align2::RIGHT_TOP,
        local_dt(note.created).format("%H:%M").to_string(),
        mono(10.5),
        theme.muted,
    );
    resp
}
