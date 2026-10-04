use eframe::egui::{self, Color32, FontData, FontDefinitions, FontFamily, FontId, Rounding, Stroke, Vec2};

// ── Nothing Theme Palettes ───────────────────────────────────────────────────
pub struct Theme {
    pub dark: bool,
    pub bg: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub border: Color32,
    pub hover: Color32,
    pub accent: Color32,
    pub red: Color32,
    /// Very subtle colour used for the dot-matrix texture.
    pub dots: Color32,
}

impl Theme {
    pub fn get(dark: bool) -> Self {
        if dark {
            Self {
                dark,
                bg: Color32::from_rgb(0, 0, 0),           // Pure black
                text: Color32::from_rgb(255, 255, 255),   // Pure white
                muted: Color32::from_rgb(190, 190, 190),  // Still clearly readable
                border: Color32::from_rgb(58, 58, 58),
                hover: Color32::from_rgb(26, 26, 26),
                accent: Color32::from_rgb(255, 255, 255),
                red: Color32::from_rgb(235, 45, 45),      // Nothing red
                dots: Color32::from_rgb(34, 34, 34),
            }
        } else {
            Self {
                dark,
                bg: Color32::from_rgb(255, 255, 255),     // Pure white
                text: Color32::from_rgb(0, 0, 0),         // Pure black
                muted: Color32::from_rgb(85, 85, 85),
                border: Color32::from_rgb(215, 215, 215),
                hover: Color32::from_rgb(242, 242, 242),
                accent: Color32::from_rgb(0, 0, 0),
                red: Color32::from_rgb(215, 25, 32),
                dots: Color32::from_rgb(228, 228, 228),
            }
        }
    }
}

// ── Fonts ───────────────────────────────────────────────────────────────────

pub const DOTO: &str = "doto";

/// Dot-matrix display font used for headings and the idle notch.
pub fn doto(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(DOTO.into()))
}

/// Space Mono, used for small uppercase meta labels.
pub fn mono(size: f32) -> FontId {
    FontId::monospace(size)
}

pub fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    fonts
        .font_data
        .insert(DOTO.into(), FontData::from_static(include_bytes!("../assets/fonts/Doto.ttf")));
    fonts.font_data.insert(
        "space_mono".into(),
        FontData::from_static(include_bytes!("../assets/fonts/SpaceMono-Regular.ttf")),
    );

    // Doto first, then fall back to the default proportional stack (emoji/icons).
    let mut doto_stack = vec![DOTO.to_owned()];
    doto_stack.extend(fonts.families.get(&FontFamily::Proportional).cloned().unwrap_or_default());
    fonts.families.insert(FontFamily::Name(DOTO.into()), doto_stack);

    if let Some(mono) = fonts.families.get_mut(&FontFamily::Monospace) {
        mono.insert(0, "space_mono".into());
    }
    ctx.set_fonts(fonts);
}

// ── Style ───────────────────────────────────────────────────────────────────

pub fn setup_style(ctx: &egui::Context, theme: &Theme) {
    let mut style = (*ctx.style()).clone();
    style.spacing.scroll = egui::style::ScrollStyle::thin();
    style.spacing.menu_margin = egui::Margin::same(6.0);
    style.spacing.button_padding = Vec2::new(10.0, 5.0);
    style.spacing.item_spacing = Vec2::new(6.0, 2.0);

    let v = &mut style.visuals;
    *v = if theme.dark { egui::Visuals::dark() } else { egui::Visuals::light() };
    v.panel_fill = Color32::TRANSPARENT;
    v.window_fill = theme.bg;
    v.extreme_bg_color = theme.bg;
    v.faint_bg_color = theme.hover;
    v.window_stroke = Stroke::new(1.0_f32, theme.border);
    v.window_rounding = Rounding::same(12.0);
    v.menu_rounding = Rounding::same(12.0);
    v.window_shadow = egui::epaint::Shadow::NONE;
    v.popup_shadow = egui::epaint::Shadow::NONE;
    let time = ctx.input(|i| i.time);
    let blink_on = (time * 1.8).fract() < 0.55;
    let cursor_color = if blink_on { theme.red } else { Color32::TRANSPARENT };
    v.text_cursor = Stroke::new(1.5_f32, cursor_color);
    v.selection.bg_fill = theme.red.linear_multiply(0.35);
    v.selection.stroke = Stroke::new(1.0_f32, theme.text);
    v.hyperlink_color = theme.red;

    let w = &mut v.widgets;
    for (wv, fill) in [
        (&mut w.noninteractive, Color32::TRANSPARENT),
        (&mut w.inactive, Color32::TRANSPARENT),
        (&mut w.hovered, theme.hover),
        (&mut w.active, theme.hover),
        (&mut w.open, theme.hover),
    ] {
        wv.bg_fill = fill;
        wv.weak_bg_fill = fill;
        wv.bg_stroke = Stroke::NONE;
        wv.fg_stroke = Stroke::new(1.0_f32, theme.text);
        wv.rounding = Rounding::same(8.0);
        wv.expansion = 0.0;
    }
    w.noninteractive.bg_stroke = Stroke::new(1.0_f32, theme.border); // separators
    ctx.set_style(style);
}
