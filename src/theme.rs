//! Dark and Light themes: the same colours as ProxyCraft 1.x.

use eframe::egui::{self, Color32, CornerRadius, Stroke, Visuals};

#[derive(Clone, Copy)]
pub struct Colors {
    pub window: Color32,
    pub panel: Color32,
    pub panel_alt: Color32,
    pub field: Color32,
    pub status: Color32,
    pub text: Color32,
    pub text_soft: Color32,
    pub muted: Color32,
    pub accent: Color32,
    pub accent_soft: Color32,
    pub border: Color32,
    pub border_soft: Color32,
    pub button: Color32,
    pub hover: Color32,
    pub pressed: Color32,
    pub donate_border: Color32,
    pub done: Color32,
    pub failed: Color32,
}

const fn hex(v: u32) -> Color32 {
    Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

pub const DARK: Colors = Colors {
    window: hex(0x11151b),
    panel: hex(0x171c23),
    panel_alt: hex(0x1b2430),
    field: hex(0x0d1117),
    status: hex(0x0b0e12),
    text: hex(0xe5e9ef),
    text_soft: hex(0xd8dee8),
    muted: hex(0x8e9aaa),
    accent: hex(0x8eb3dc),
    accent_soft: hex(0x9fb7d2),
    border: hex(0x303946),
    border_soft: hex(0x394454),
    button: hex(0x222a35),
    hover: hex(0x2b3543),
    pressed: hex(0x354154),
    donate_border: hex(0x354b64),
    done: hex(0x7fc98f),
    failed: hex(0xe58a8a),
};

pub const LIGHT: Colors = Colors {
    window: hex(0xeef2f6),
    panel: hex(0xffffff),
    panel_alt: hex(0xe5ebf2),
    field: hex(0xffffff),
    status: hex(0xe3e9f0),
    text: hex(0x1b2733),
    text_soft: hex(0x273746),
    muted: hex(0x5e6d7d),
    accent: hex(0x2d6696),
    accent_soft: hex(0x3a719e),
    border: hex(0xc4ced9),
    border_soft: hex(0xaebbc9),
    button: hex(0xe2e9f0),
    hover: hex(0xd2dfe9),
    pressed: hex(0xc5d4e0),
    donate_border: hex(0x9eafc0),
    done: hex(0x2e7d44),
    failed: hex(0xb23a3a),
};

pub fn colors(theme: &str) -> Colors {
    if theme == "light" {
        LIGHT
    } else {
        DARK
    }
}

pub fn apply(ctx: &egui::Context, theme: &str) {
    let c = colors(theme);
    let mut v = if theme == "light" {
        Visuals::light()
    } else {
        Visuals::dark()
    };
    v.override_text_color = Some(c.text);
    v.window_fill = c.panel;
    v.panel_fill = c.window;
    v.extreme_bg_color = c.field;
    v.faint_bg_color = c.panel;
    v.code_bg_color = c.panel_alt;
    v.hyperlink_color = c.accent;
    v.window_stroke = Stroke::new(1.0, c.border);
    v.selection.bg_fill = c.hover;
    v.selection.stroke = Stroke::new(1.0, c.accent);
    let r = CornerRadius::same(4);
    for (w, fill) in [
        (&mut v.widgets.noninteractive, c.window),
        (&mut v.widgets.inactive, c.button),
        (&mut v.widgets.hovered, c.hover),
        (&mut v.widgets.active, c.pressed),
        (&mut v.widgets.open, c.hover),
    ] {
        w.bg_fill = fill;
        w.weak_bg_fill = fill;
        w.corner_radius = r;
        w.fg_stroke = Stroke::new(1.0, c.text);
    }
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, c.border);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, c.text_soft);
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, c.border_soft);
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, c.accent);
    v.widgets.active.bg_stroke = Stroke::new(1.0, c.accent);
    ctx.set_visuals(v);
    ctx.global_style_mut(|s| {
        s.spacing.button_padding = egui::vec2(12.0, 6.0);
        s.spacing.item_spacing = egui::vec2(8.0, 6.0);
        s.spacing.interact_size.y = 26.0;
        use egui::{FontFamily, FontId, TextStyle};
        s.text_styles.insert(
            TextStyle::Small,
            FontId::new(12.0, FontFamily::Proportional),
        );
        s.text_styles
            .insert(TextStyle::Body, FontId::new(14.0, FontFamily::Proportional));
        s.text_styles.insert(
            TextStyle::Button,
            FontId::new(14.0, FontFamily::Proportional),
        );
    });
}
