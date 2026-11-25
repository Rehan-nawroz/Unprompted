//! Unprompted visual theme: brand palette, fonts and widget styling.
//!
//! Colours follow the design mockup (Tailwind tokens):
//! brand violet `#6344f5..#8b5cf6`, coral `#ff5b5b`, emerald `#34d399`,
//! surfaces `#0B0914` app / `#131124` card / `#1A1733` panel.

use eframe::egui::{self, Color32, FontDefinitions, FontFamily, FontId, TextStyle, Visuals};

// Surfaces.
pub const TITLEBAR: Color32 = Color32::from_rgb(0x07, 0x06, 0x0E);
pub const BG: Color32 = Color32::from_rgb(0x0B, 0x09, 0x14);
pub const CARD: Color32 = Color32::from_rgb(0x13, 0x11, 0x24);
pub const PANEL: Color32 = Color32::from_rgb(0x1A, 0x17, 0x33);
pub const HOVER: Color32 = Color32::from_rgb(0x22, 0x1E, 0x42);
pub const BORDER: Color32 = Color32::from_rgb(0x28, 0x24, 0x4C);
pub const BORDER_SUBTLE: Color32 = Color32::from_rgb(0x1F, 0x1C, 0x3B);

// Brand violet.
pub const BRAND_300: Color32 = Color32::from_rgb(0xA6, 0x8B, 0xFA);
pub const BRAND_400: Color32 = Color32::from_rgb(0x8B, 0x5C, 0xF6);
pub const BRAND_500: Color32 = Color32::from_rgb(0x70, 0x47, 0xEB);
pub const BRAND_600: Color32 = Color32::from_rgb(0x63, 0x44, 0xF5);
pub const BRAND_700: Color32 = Color32::from_rgb(0x53, 0x34, 0xDB);
pub const GLOW: Color32 = Color32::from_rgb(0x78, 0x57, 0xFF);

// Coral accent.
pub const CORAL_300: Color32 = Color32::from_rgb(0xFF, 0x9A, 0x9A);
pub const CORAL_400: Color32 = Color32::from_rgb(0xFF, 0x73, 0x73);
pub const CORAL_500: Color32 = Color32::from_rgb(0xFF, 0x5B, 0x5B);
pub const CORAL_600: Color32 = Color32::from_rgb(0xF0, 0x43, 0x43);

// Status green.
pub const EMERALD: Color32 = Color32::from_rgb(0x34, 0xD3, 0x99);
pub const EMERALD_DIM: Color32 = Color32::from_rgb(0x10, 0x63, 0x4B);

// Text.
pub const TEXT: Color32 = Color32::from_rgb(0xE2, 0xE8, 0xF0); // slate-200
pub const MUTED: Color32 = Color32::from_rgb(0x94, 0xA3, 0xB8); // slate-400
pub const FAINT: Color32 = Color32::from_rgb(0x64, 0x74, 0x8B); // slate-500
pub const DIM: Color32 = Color32::from_rgb(0x47, 0x55, 0x69); // slate-600


/// Font family for semi-bold text ("JakartaSemi").
pub const SEMI: &str = "JakartaSemi";
/// Font family for extra-bold text ("JakartaXB").
pub const XBOLD: &str = "JakartaXB";
/// Font family for semi-bold mono ("MonoBold").
pub const MONO_B: &str = "MonoBold";

const MONO_FONT: &[u8] = include_bytes!("../assets/fonts/Mono-Regular.ttf");
const MONO_BOLD_FONT: &[u8] = include_bytes!("../assets/fonts/Mono-SemiBold.ttf");
const JAKARTA_REG: &[u8] = include_bytes!("../assets/fonts/Jakarta-Regular.ttf");
const JAKARTA_SEMI: &[u8] = include_bytes!("../assets/fonts/Jakarta-SemiBold.ttf");
const JAKARTA_XB: &[u8] = include_bytes!("../assets/fonts/Jakarta-ExtraBold.ttf");

pub fn apply(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    for (name, data) in [
        ("Jakarta", JAKARTA_REG),
        (SEMI, JAKARTA_SEMI),
        (XBOLD, JAKARTA_XB),
        ("Mono", MONO_FONT),
        (MONO_B, MONO_BOLD_FONT),
    ] {
        fonts.font_data.insert(name.to_owned(), egui::FontData::from_static(data).into());
    }
    // Brand fonts first; default egui fonts stay as glyph fallbacks.
    fonts
        .families
        .get_mut(&FontFamily::Proportional)
        .unwrap()
        .insert(0, "Jakarta".to_owned());
    fonts.families.insert(FontFamily::Name(SEMI.into()), vec!["JakartaSemi".into(), "Jakarta".into()]);
    fonts.families.insert(FontFamily::Name(XBOLD.into()), vec!["JakartaXB".into(), "Jakarta".into()]);
    fonts
        .families
        .get_mut(&FontFamily::Monospace)
        .unwrap()
        .insert(0, "Mono".to_owned());
    fonts.families.insert(FontFamily::Name(MONO_B.into()), vec!["MonoBold".into(), "Mono".into()]);
    ctx.set_fonts(fonts);

    let mut visuals = Visuals::dark();
    visuals.panel_fill = BG;
    visuals.window_fill = CARD;
    visuals.extreme_bg_color = TITLEBAR;
    visuals.faint_bg_color = PANEL;
    visuals.selection.stroke = egui::Stroke::new(1.0, BRAND_500);
    visuals.selection.bg_fill = BRAND_600.gamma_multiply(0.35);
    visuals.widgets.hovered.bg_fill = HOVER;
    visuals.widgets.active.bg_fill = HOVER;
    visuals.widgets.inactive.bg_fill = PANEL;
    visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, BORDER);
    visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, BRAND_500);
    visuals.widgets.active.bg_stroke = egui::Stroke::new(1.0, BRAND_400);
    visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, BORDER);
    visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, MUTED);
    visuals.menu_corner_radius = egui::CornerRadius::same(10);
    ctx.set_visuals(visuals.clone());

    ctx.global_style_mut(|style| {
        style.visuals = visuals;
        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
        style.spacing.button_padding = egui::vec2(10.0, 6.0);
        style.spacing.menu_margin = egui::Margin::symmetric(6, 6);
        style.spacing.interact_size = egui::vec2(40.0, 28.0);
        style.text_styles = [
            (TextStyle::Heading, FontId::new(17.0, FontFamily::Name(XBOLD.into()))),
            (TextStyle::Body, FontId::new(13.0, FontFamily::Proportional)),
            (TextStyle::Button, FontId::new(12.5, FontFamily::Name(SEMI.into()))),
            (TextStyle::Small, FontId::new(10.5, FontFamily::Proportional)),
            (TextStyle::Monospace, FontId::new(12.0, FontFamily::Monospace)),
        ]
        .into();
    });
}

/// Linear interpolation between two colours, `t` in 0..1.
pub fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let mix = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgba_premultiplied(
        mix(a.r(), b.r()),
        mix(a.g(), b.g()),
        mix(a.b(), b.b()),
        mix(a.a(), b.a()),
    )
}
