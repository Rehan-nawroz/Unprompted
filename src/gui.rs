//! Unprompted window, rebuilt from the design mockup:
//! title strip, brand header, hero drop zone, filter tabs, rich result cards with a
//! purged-elements drawer, and a status footer. Files are cleaned as soon as they are
//! dropped or chosen. Originals are never modified.

use std::path::{Path, PathBuf};

use eframe::egui::{
    self, pos2, Align2, Color32, FontFamily, FontId, Margin, Painter, Rect, RichText, Sense, Stroke,
};
use unprompted::{clean_file_with, expand_inputs, write_zip, Format, Kind, Profile, Removal};

use crate::icons;
use crate::theme::{
    self, lerp_color, BORDER, BORDER_SUBTLE, BRAND_300, BRAND_400, BRAND_500, BRAND_600, BRAND_700,
    BG, CARD, CORAL_300, CORAL_400, CORAL_500, CORAL_600, DIM, EMERALD, EMERALD_DIM, FAINT, GLOW,
    HOVER, MUTED, PANEL, TEXT, TITLEBAR,
};



const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "jpe", "jfif", "webp"];

/// Brand logo, pre-decoded to raw RGBA with rounded corners (128×128).
const LOGO_RGBA: &[u8] = include_bytes!("../assets/logo.rgba");
/// Window/taskbar icon source (256×256 RGBA).
const ICON_RGBA: &[u8] = include_bytes!("../assets/icon256.rgba");

pub fn run(initial: Vec<PathBuf>) -> eframe::Result {
    let icon = egui::IconData {
        width: 256,
        height: 256,
        rgba: ICON_RGBA.to_vec(),
    };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Unprompted")
            .with_inner_size([1060.0, 720.0])
            .with_min_inner_size([900.0, 600.0])
            .with_drag_and_drop(true)
            .with_icon(icon),
        ..Default::default()
    };
    eframe::run_native(
        "Unprompted",
        options,
        Box::new(move |cc| {
            theme::apply(&cc.egui_ctx);
            let mut app = App::default();
            let image = egui::ColorImage::from_rgba_unmultiplied([128, 128], LOGO_RGBA);
            app.logo = Some(cc.egui_ctx.load_texture("brand-logo", image, egui::TextureOptions::LINEAR));
            app.add_paths(&initial);
            Ok(Box::new(app))
        }),
    )
}

// ---------------------------------------------------------------- text helpers

fn mono(size: f32, text: impl Into<String>, color: Color32) -> RichText {
    RichText::new(text.into()).size(size).color(color).family(FontFamily::Monospace)
}

fn mono_b(size: f32, text: impl Into<String>, color: Color32) -> RichText {
    RichText::new(text.into()).size(size).color(color).family(FontFamily::Name(theme::MONO_B.into()))
}

fn semi(size: f32, text: impl Into<String>, color: Color32) -> RichText {
    RichText::new(text.into()).size(size).color(color).family(FontFamily::Name(theme::SEMI.into()))
}

fn width_of(ui: &Ui, text: &str, font: FontId) -> f32 {
    ui.painter().layout_no_wrap(text.to_owned(), font, Color32::WHITE).size().x
}

use eframe::egui::Ui;

// ---------------------------------------------------------------- state

/// Successful clean of one file.
struct Report {
    output: PathBuf,
    format: Format,
    original: u64,
    cleaned: u64,
    removals: Vec<Removal>,
}

impl Report {
    fn saved(&self) -> u64 {
        self.original.saturating_sub(self.cleaned)
    }

    fn has_ai_data(&self) -> bool {
        self.removals.iter().any(|r| r.kind == Kind::AiData)
    }
}

struct Item {
    source: PathBuf,
    seq: u64,
    result: Result<Report, String>,
    drawer_open: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    All,
    Cleaned,
    Issues,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Sort {
    Added,
    Name,
    Size,
}

impl Sort {
    fn next(self) -> Self {
        match self {
            Sort::Added => Sort::Name,
            Sort::Name => Sort::Size,
            Sort::Size => Sort::Added,
        }
    }
}

pub struct App {
    items: Vec<Item>,
    profile: Profile,
    tab: Tab,
    filter: String,
    sort: Sort,
    /// Global "Detail View" switch: opens the purged-elements drawer on every card.
    show_details: bool,
    next_seq: u64,
    /// Brand logo texture for the header.
    logo: Option<egui::TextureHandle>,
    /// Popup state for the header dropdowns (manual, so styling matches the mockup).
    profile_open: bool,
    about_open: bool,
    profile_btn: Rect,
    about_btn: Rect,
    profile_menu: Rect,
    about_menu: Rect,
}

impl Default for App {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            profile: Profile::Full,
            tab: Tab::All,
            filter: String::new(),
            sort: Sort::Added,
            show_details: false,
            next_seq: 0,
            logo: None,
            profile_open: false,
            about_open: false,
            profile_btn: Rect::ZERO,
            about_btn: Rect::ZERO,
            profile_menu: Rect::ZERO,
            about_menu: Rect::ZERO,
        }
    }
}

enum Action {
    Reveal(PathBuf),
    Remove(usize),
    Export,
    ToggleDrawer(usize),
}

impl App {
    /// Cleans the given files and folders. A file already in the list is refreshed in place.
    fn add_paths(&mut self, paths: &[PathBuf]) {
        for source in expand_inputs(paths) {
            let result = match clean_file_with(&source, self.profile) {
                Ok(o) => Ok(Report {
                    output: o.output.clone(),
                    format: o.cleaned.format,
                    original: o.original_size,
                    cleaned: o.cleaned_size(),
                    removals: o.cleaned.removed.clone(),
                }),
                Err(e) => Err(e.to_string()),
            };
            let seq = self.next_seq;
            self.next_seq += 1;
            let item = Item { source: source.clone(), seq, result, drawer_open: false };
            match self.items.iter().position(|i| i.source == source) {
                Some(pos) => self.items[pos] = item,
                None => self.items.push(item),
            }
        }
    }

    /// Re-cleans everything currently in the list with the new profile.
    fn apply_profile(&mut self, profile: Profile) {
        if profile == self.profile {
            return;
        }
        self.profile = profile;
        let sources: Vec<PathBuf> = self.items.iter().map(|i| i.source.clone()).collect();
        self.items.clear();
        self.add_paths(&sources);
    }

    fn browse(&mut self) {
        let picked = rfd::FileDialog::new()
            .set_title("Choose images to clean")
            .add_filter("Images", IMAGE_EXTENSIONS)
            .pick_files();
        if let Some(paths) = picked {
            self.add_paths(&paths);
        }
    }

    fn export_all(&mut self) {
        let outputs: Vec<PathBuf> = self
            .items
            .iter()
            .filter_map(|i| i.result.as_ref().ok().map(|r| r.output.clone()))
            .collect();
        if outputs.is_empty() {
            return;
        }
        let Some(target) = rfd::FileDialog::new()
            .set_title("Export cleaned images")
            .set_file_name("unprompted_clean.zip")
            .add_filter("ZIP archive", &["zip"])
            .save_file()
        else {
            return;
        };
        let mut entries: Vec<(String, Vec<u8>)> = Vec::new();
        for path in &outputs {
            if let Ok(bytes) = std::fs::read(path) {
                entries.push((name_of(path), bytes));
            }
        }
        if let Err(e) =
            std::fs::File::create(&target).and_then(|mut f| write_zip(&entries, &mut f))
        {
            self.items.push(Item {
                source: target,
                seq: self.next_seq,
                result: Err(format!("export failed: {e}")),
                drawer_open: false,
            });
            self.next_seq += 1;
        }
    }

    fn counts(&self) -> (usize, usize) {
        self.items.iter().fold((0, 0), |(ok, err), i| match &i.result {
            Ok(_) => (ok + 1, err),
            Err(_) => (ok, err + 1),
        })
    }

    fn purged_bytes(&self) -> u64 {
        self.items.iter().fold(0u64, |sum, i| match &i.result {
            Ok(r) => sum + r.saved(),
            Err(_) => sum,
        })
    }

    /// Indices into `self.items` after applying the active tab, text filter and sort.
    fn visible(&self) -> Vec<usize> {
        let mut idx: Vec<usize> = self
            .items
            .iter()
            .enumerate()
            .filter(|(_, i)| {
                let ok = i.result.is_ok();
                match self.tab {
                    Tab::All => true,
                    Tab::Cleaned => ok,
                    Tab::Issues => !ok,
                }
            })
            .filter(|(_, i)| {
                let q = self.filter.trim().to_ascii_lowercase();
                q.is_empty()
                    || name_of(&i.source).to_ascii_lowercase().contains(&q)
            })
            .map(|(i, _)| i)
            .collect();
        match self.sort {
            Sort::Added => idx.sort_by_key(|&i| std::cmp::Reverse(self.items[i].seq)),
            Sort::Name => idx.sort_by(|&a, &b| {
                name_of(&self.items[a].source).cmp(&name_of(&self.items[b].source))
            }),
            Sort::Size => idx.sort_by_key(|&i| match &self.items[i].result {
                Ok(r) => std::cmp::Reverse(r.cleaned),
                Err(_) => std::cmp::Reverse(0u64),
            }),
        }
        idx
    }
}

fn name_of(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

// ---------------------------------------------------------------- frame

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        let dropped: Vec<PathBuf> = ctx
            .input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).collect());
        if !dropped.is_empty() {
            self.add_paths(&dropped);
        }
        let hovering = ctx.input(|i| !i.raw.hovered_files.is_empty());
        let ctrl_o = ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::O));

        let mut actions: Vec<Action> = Vec::new();
        if ctrl_o {
            self.browse();
        }
        self.close_popups_on_outside_click(&ctx);

        egui::Panel::top("titlebar")
            .exact_size(26.0)
            .frame(egui::Frame::NONE.fill(TITLEBAR).inner_margin(Margin::symmetric(14, 0)))
            .show(ui, |ui| self.titlebar(ui));

        egui::Panel::top("header")
            .exact_size(64.0)
            .frame(
                egui::Frame::NONE
                    .fill(CARD)
                    .stroke(Stroke::new(1.0, BORDER_SUBTLE))
                    .inner_margin(Margin::symmetric(20, 10)),
            )
            .show(ui, |ui| self.header(ui));

        egui::Panel::bottom("footer")
            .exact_size(46.0)
            .frame(
                egui::Frame::NONE
                    .fill(TITLEBAR)
                    .stroke(Stroke::new(1.0, BORDER_SUBTLE))
                    .inner_margin(Margin::symmetric(16, 0)),
            )
            .show(ui, |ui| self.footer(ui, &mut actions));

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(BG).inner_margin(Margin::same(16)))
            .show(ui, |ui| {
                if self.items.is_empty() {
                    self.hero(ui, true, hovering);
                    ui.add_space(14.0);
                    self.toolbar(ui);
                    ui.add_space(10.0);
                    empty_hint(ui);
                } else {
                    self.hero(ui, false, hovering);
                    ui.add_space(14.0);
                    self.toolbar(ui);
                    ui.add_space(10.0);
                    if self.visible().is_empty() {
                        // Items exist, but the active tab + filter hides them all.
                        ui.vertical_centered(|ui| {
                            ui.add_space(18.0);
                            let (r, _) =
                                ui.allocate_exact_size(egui::vec2(30.0, 30.0), Sense::hover());
                            icons::search_off(ui.painter(), r, DIM);
                            ui.add_space(4.0);
                            ui.label(semi(13.0, "Nothing matches here", MUTED));
                            let hint = if self.filter.trim().is_empty() {
                                "No files in this tab yet.".to_string()
                            } else {
                                format!(
                                    "No files matching \u{201c}{}\u{201d} in this tab.",
                                    self.filter.trim()
                                )
                            };
                            ui.label(RichText::new(hint).size(11.0).color(FAINT));
                            ui.add_space(6.0);
                            if panel_button(ui, "Show all files", icons::search).clicked() {
                                self.filter.clear();
                                self.tab = Tab::All;
                            }
                        });
                    } else {
                        egui::ScrollArea::vertical()
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                for idx in self.visible() {
                                    let item = &self.items[idx];
                                    if let Some(action) =
                                        result_card(ui, idx, item, self.show_details)
                                    {
                                        actions.push(action);
                                    }
                                    ui.add_space(10.0);
                                }
                            });
                    }
                }
            });

        for action in actions {
            self.handle(action);
        }
    }
}

// ---------------------------------------------------------------- title strip + header

impl App {
    fn titlebar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_centered(|ui| {
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                let (r, _) = ui.allocate_exact_size(egui::vec2(12.0, 12.0), Sense::hover());
                icons::dot(ui.painter(), r, BRAND_400);
                ui.add_space(2.0);
                ui.label(mono(9.0, "unprompted.exe · portable build", FAINT));
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(mono(9.0, "v1.0", DIM));
                ui.add_space(8.0);
                session_chip(ui, "LOCAL · OFFLINE");
                ui.add_space(6.0);
                ui.label(RichText::new("/").color(FAINT).size(10.0));
                ui.add_space(6.0);
                ui.label(semi(10.5, "Unprompted Studio", MUTED));
            });
        });
    }

    fn header(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_centered(|ui| {
            let (tile, _) = ui.allocate_exact_size(egui::vec2(40.0, 40.0), Sense::hover());
            draw_logo(ui.painter(), tile, self.logo.as_ref());

            ui.add_space(4.0);
            ui.vertical(|ui| {
                ui.label(
                    RichText::new("Unprompted")
                        .size(17.0)
                        .color(Color32::WHITE)
                        .family(FontFamily::Name(theme::XBOLD.into())),
                );
                ui.label(
                    RichText::new("Strip AI Prompts, C2PA Credentials, EXIF, GPS & Hidden Markers")
                        .size(11.0)
                        .color(MUTED),
                );
            });

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(2.0);
                if gradient_button(ui, "Add Images or Folders").clicked() {
                    self.browse();
                }

                ui.add_space(8.0);
                // About / advanced-rules popup.
                let about_btn = panel_button(ui, "Advanced", icons::sliders);
                self.about_btn = about_btn.rect;
                if about_btn.clicked() {
                    self.about_open = !self.about_open;
                    self.profile_open = false;
                }
                if self.about_open {
                    self.about_popup(ui);
                }

                ui.add_space(8.0);
                // Scrub profile dropdown.
                let label = format!("Profile:  {}", self.profile.label());
                let pro_btn = panel_button(ui, &label, icons::shield);
                self.profile_btn = pro_btn.rect;
                if pro_btn.clicked() {
                    self.profile_open = !self.profile_open;
                    self.about_open = false;
                }
                if self.profile_open {
                    self.profile_popup(ui);
                }
            });
        });
    }

    /// Dropdown listing the three scrub profiles.
    fn profile_popup(&mut self, ui: &mut egui::Ui) {
        let menu_w = 300.0;
        let mut picked: Option<Profile> = None;
        let response = egui::Area::new(egui::Id::unique("profile_menu"))
            .fixed_pos(pos2(self.profile_btn.right() - menu_w, self.profile_btn.bottom() + 6.0))
            .order(egui::Order::Foreground)
            .show(ui.ctx(), |ui| {
                egui::Frame::NONE
                    .fill(CARD)
                    .stroke(Stroke::new(1.0, BORDER))
                    .corner_radius(12.0)
                    .shadow(egui::Shadow::NONE)
                    .inner_margin(Margin::same(8))
                    .show(ui, |ui| {
                        ui.set_min_width(menu_w - 16.0);
                        ui.label(mono(9.5, "SCRUB PROFILE", FAINT));
                        ui.add_space(2.0);
                        for p_profile in Profile::all() {
                            let selected = p_profile == self.profile;
                            let (row, resp) =
                                ui.allocate_exact_size(egui::vec2(ui.available_width(), 26.0), Sense::click());
                            let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
                            let p_ = ui.painter();
                            if selected || resp.hovered() {
                                p_.rect_filled(row, 8.0, if selected { BRAND_600.gamma_multiply(0.25) } else { HOVER });
                            }
                            if selected {
                                icons::check(p_, Rect::from_center_size(pos2(row.left() + 14.0, row.center().y), egui::vec2(11.0, 11.0)), BRAND_300);
                            }
                            p_.text(pos2(row.left() + 30.0, row.center().y), Align2::LEFT_CENTER, p_profile.label(), FontId::new(12.0, FontFamily::Name(theme::SEMI.into())), if selected { BRAND_300 } else { TEXT });
                            if resp.clicked() {
                                picked = Some(p_profile);
                            }
                            ui.label(RichText::new(p_profile.description()).size(10.5).color(MUTED));
                            ui.add_space(4.0);
                        }
                    });
            });
        self.profile_menu = response.response.rect;
        if let Some(p) = picked {
            self.apply_profile(p);
            self.profile_open = false;
        }
    }

    /// Static info popup for the "Advanced" button.
    fn about_popup(&mut self, ui: &mut egui::Ui) {
        let menu_w = 300.0;
        let response = egui::Area::new(egui::Id::unique("about_menu"))
            .fixed_pos(pos2(self.about_btn.right() - menu_w, self.about_btn.bottom() + 6.0))
            .order(egui::Order::Foreground)
            .show(ui.ctx(), |ui| {
                egui::Frame::NONE
                    .fill(CARD)
                    .stroke(Stroke::new(1.0, BORDER))
                    .corner_radius(12.0)
                    .inner_margin(Margin::same(10))
                    .show(ui, |ui| {
                        ui.set_min_width(menu_w - 20.0);
                        ui.label(semi(12.5, "Unprompted v1.0", Color32::WHITE));
                        ui.label(RichText::new("Portable metadata cleaner — no installer, no config, no cloud.").size(11.0).color(MUTED));
                        ui.add_space(8.0);
                        ui.label(mono(9.5, "SUPPORTED FORMATS", FAINT));
                        ui.label(mono(11.0, "PNG · JPEG · WebP", TEXT));
                        ui.add_space(8.0);
                        ui.label(mono(9.5, "ENGINE", FAINT));
                        ui.label(RichText::new("Byte-exact whitelist stripper. Pixel data is copied, never re-encoded.").size(11.0).color(MUTED));
                        ui.add_space(8.0);
                        ui.label(mono(9.5, "HEADLESS MODE", FAINT));
                        ui.label(mono(10.5, "Unprompted.exe --cli <files...>", TEXT));
                    });
            });
        self.about_menu = response.response.rect;
    }

    /// Closes open popups when the user clicks anywhere outside them.
    fn close_popups_on_outside_click(&mut self, ctx: &egui::Context) {
        if !self.profile_open && !self.about_open {
            return;
        }
        let clicked = ctx.input(|i| i.pointer.any_click());
        if !clicked {
            return;
        }
        let pos = ctx.input(|i| i.pointer.interact_pos());
        let inside = |r: Rect| pos.is_some_and(|p| r.contains(p));
        let inside_any = inside(self.profile_btn)
            || inside(self.about_btn)
            || inside(self.profile_menu)
            || inside(self.about_menu);
        if !inside_any {
            self.profile_open = false;
            self.about_open = false;
        }
    }

    fn footer(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        let (ok, err) = self.counts();
        let purged = self.purged_bytes();

        ui.horizontal_centered(|ui| {
            // Privacy assurance with a live-looking status dot.
            let (dot, _) = ui.allocate_exact_size(egui::vec2(14.0, 14.0), Sense::hover());
            let p = ui.painter();
            p.circle_filled(dot.center(), 5.0, EMERALD);
            p.circle_stroke(dot.center(), 7.0, Stroke::new(1.5, EMERALD.gamma_multiply(0.35)));
            ui.label(
                RichText::new("Works offline on this PC. Pixels are copied byte for byte.")
                    .size(11.5)
                    .color(TEXT),
            );

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Clear list.
                if clear_button(ui).clicked() {
                    self.items.clear();
                    self.tab = Tab::All;
                    self.filter.clear();
                }
                ui.add_space(6.0);
                // Export All (.zip).
                if panel_button(ui, "Export All (.zip)", icons::file_zipper).clicked() {
                    actions.push(Action::Export);
                }
                ui.add_space(12.0);

                // Queue metrics, added right-to-left so they read left-to-right.
                ui.label(mono(10.5, "purged", MUTED));
                ui.add_space(3.0);
                ui.label(mono_b(10.5, human_size(purged), EMERALD));
                ui.add_space(6.0);
                ui.label(RichText::new("|").color(FAINT).size(11.0));
                ui.add_space(6.0);
                if err > 0 {
                    ui.label(mono(10.5, "issues", MUTED));
                    ui.add_space(3.0);
                    ui.label(mono_b(10.5, format!("{err}"), CORAL_400));
                    ui.add_space(3.0);
                    ui.label(RichText::new("/").color(FAINT).size(11.0));
                    ui.add_space(3.0);
                }
                ui.add_space(2.0);
                ui.label(mono(10.5, "cleaned", MUTED));
                ui.add_space(3.0);
                ui.label(mono_b(10.5, format!("{ok}"), Color32::WHITE));
            });
        });
    }
}

/// The violet→coral gradient primary button.
fn gradient_button(ui: &mut egui::Ui, label: &str) -> egui::Response {
    let height = 32.0;
    let pad = 16.0;
    let icon_space = 20.0;
    let font = FontId::new(12.0, FontFamily::Name(theme::SEMI.into()));
    let text_w = width_of(ui, label, font.clone());
    let size = egui::vec2(text_w + pad * 2.0 + icon_space, height);
    let (rect, mut response) = ui.allocate_exact_size(size, Sense::click());
    response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
    let hover = response.hovered() || response.dragged();
    let p = ui.painter();

    // Glow: a soft halo behind the button.
    p.rect_filled(
        rect.expand(3.0),
        10.0,
        BRAND_600.gamma_multiply(if hover { 0.35 } else { 0.22 }),
    );
    // Gradient border: vertical strips from brand-600 to coral-500.
    let strips = 24;
    let border = 1.25;
    for s in 0..strips {
        let t = s as f32 / (strips - 1) as f32;
        let x0 = rect.left() + rect.width() * s as f32 / strips as f32;
        let x1 = rect.left() + rect.width() * (s + 1) as f32 / strips as f32;
        p.rect_filled(
            Rect::from_min_max(pos2(x0, rect.top()), pos2(x1, rect.bottom())),
            0.0,
            lerp_color(BRAND_600, CORAL_500, t),
        );
    }
    // Inner gradient fill brand-600 → brand-700.
    for s in 0..strips {
        let t = s as f32 / (strips - 1) as f32;
        let x0 = rect.left() + border + rect.width() * s as f32 / strips as f32 - (s as f32) * 0.001;
        let x1 = rect.left() + border + rect.width() * (s + 1) as f32 / strips as f32;
        p.rect_filled(
            Rect::from_min_max(
                pos2(x0, rect.top() + border),
                pos2(x1, rect.bottom() - border),
            ),
            0.0,
            lerp_color(BRAND_600, if hover { BRAND_500 } else { BRAND_700 }, t),
        );
    }
    // Content: coral plus + label.
    let icon_rect = Rect::from_center_size(
        pos2(rect.left() + pad + 5.0, rect.center().y),
        egui::vec2(12.0, 12.0),
    );
    icons::plus(p, icon_rect, CORAL_400);
    p.text(
        pos2(rect.left() + pad + icon_space, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        font,
        Color32::WHITE,
    );
    response
}


/// Panel-style button with an icon and a label.
fn panel_button(ui: &mut egui::Ui, label: &str, icon: fn(&Painter, Rect, Color32)) -> egui::Response {
    let height = 30.0;
    let font = FontId::new(11.5, FontFamily::Name(theme::SEMI.into()));
    let text_w = width_of(ui, label, font.clone());
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(text_w + 42.0, height), Sense::click());
    let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
    let hover = resp.hovered();
    let p = ui.painter();
    p.rect_filled(rect, 8.0, if hover { HOVER } else { PANEL });
    p.rect_stroke(
        rect,
        8.0,
        Stroke::new(1.0, if hover { BRAND_400 } else { BORDER }),
        egui::StrokeKind::Inside,
    );
    icon(p, Rect::from_center_size(pos2(rect.left() + 14.0, rect.center().y), egui::vec2(14.0, 14.0)), BRAND_400);
    p.text(pos2(rect.left() + 26.0, rect.center().y), Align2::LEFT_CENTER, label, font, if hover { Color32::WHITE } else { TEXT });
    resp
}

/// Ghost "Clear list" button that tints coral on hover.
fn clear_button(ui: &mut egui::Ui) -> egui::Response {
    let label = "Clear list";
    let font = FontId::new(11.5, FontFamily::Name(theme::SEMI.into()));
    let text_w = width_of(ui, label, font.clone());
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(text_w + 40.0, 30.0), Sense::click());
    let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
    let hover = resp.hovered();
    let p = ui.painter();
    if hover {
        p.rect_filled(rect, 8.0, CORAL_500.gamma_multiply(0.10));
        p.rect_stroke(rect, 8.0, Stroke::new(1.0, CORAL_500.gamma_multiply(0.4)), egui::StrokeKind::Inside);
    }
    let color = if hover { CORAL_400 } else { MUTED };
    icons::trash(p, Rect::from_center_size(pos2(rect.left() + 13.0, rect.center().y), egui::vec2(13.0, 13.0)), color);
    p.text(pos2(rect.left() + 25.0, rect.center().y), Align2::LEFT_CENTER, label, font, color);
    resp
}

/// Violet mono chip used in the title strip.
fn session_chip(ui: &mut egui::Ui, text: &str) {
    let font = FontId::new(9.0, FontFamily::Monospace);
    let w = width_of(ui, text, font.clone()) + 14.0;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(w, 18.0), Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, 4.0, BRAND_500.gamma_multiply(0.10));
    p.rect_stroke(rect, 4.0, Stroke::new(1.0, BRAND_500.gamma_multiply(0.25)), egui::StrokeKind::Inside);
    p.text(rect.center(), Align2::CENTER_CENTER, text, font, BRAND_300);
}

/// Brand mark: the real logo texture, with a soft glow ring.
fn draw_logo(p: &Painter, rect: Rect, logo: Option<&egui::TextureHandle>) {
    p.rect_stroke(rect.expand(2.0), 12.0, Stroke::new(2.0, GLOW.gamma_multiply(0.25)), egui::StrokeKind::Outside);
    if let Some(logo) = logo {
        p.image(
            logo.id(),
            rect,
            Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
            Color32::WHITE,
        );
    }
}

// ---------------------------------------------------------------- hero drop zone

impl App {
    fn hero(&mut self, ui: &mut egui::Ui, big: bool, hovering: bool) {
        let height = if big {
            176.0
        } else {
            96.0
        };
        let width = ui.available_width();
        let (rect, response) = ui.allocate_exact_size(egui::vec2(width, height), Sense::click());
        let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
        let active = hovering || response.hovered();
        let p = ui.painter_at(rect);

        // Soft outer glow + card fill + dashed violet border.
        p.rect_filled(rect.expand(2.0), 18.0, BRAND_500.gamma_multiply(if active { 0.10 } else { 0.05 }));
        p.rect_filled(rect, 16.0, if active { BRAND_500.gamma_multiply(0.06) } else { CARD });
        let outline = rounded_outline(rect.shrink(1.0), 16.0);
        let edge = if active { BRAND_400 } else { Stroke::new(1.0, BRAND_500.gamma_multiply(0.4)).color };
        dashed(&p, &outline, Stroke::new(if active { 2.0 } else { 1.5 }, edge), 9.0, 7.0);

        if big {
            // Left-ish centred layout with generous spacing.
            let icon_center = pos2(rect.left() + 64.0, rect.center().y);
            let icon_rect = Rect::from_center_size(icon_center, egui::vec2(64.0, 64.0));
            let tile = Rect::from_center_size(icon_center, egui::vec2(66.0, 66.0));
            p.rect_filled(tile, 16.0, lerp_color(BRAND_500.gamma_multiply(0.20), CORAL_500.gamma_multiply(0.10), 0.5));
            p.rect_stroke(tile, 16.0, Stroke::new(1.0, BRAND_500.gamma_multiply(0.30)), egui::StrokeKind::Inside);
            icons::cloud_upload(&p, icon_rect, if active { CORAL_400 } else { BRAND_400 });

            let text_left = rect.left() + 118.0;
            let right = rect.right() - 24.0;

            // Title + badge.
            let title_font = FontId::new(15.0, FontFamily::Name(theme::SEMI.into()));
            let badge = "FAST · LOSSLESS";
            let badge_font = FontId::new(9.0, FontFamily::Monospace);
            let title_w = p.layout_no_wrap("Drop more images or folders here".to_owned(), title_font.clone(), Color32::WHITE).size().x;
            let badge_w = p.layout_no_wrap(badge.to_owned(), badge_font.clone(), Color32::WHITE).size().x + 16.0;
            p.text(pos2(text_left, rect.center().y - 32.0), Align2::LEFT_CENTER, "Drop more images or folders here", title_font, Color32::WHITE);
            let badge_rect = Rect::from_min_size(pos2(text_left + title_w + 10.0, rect.center().y - 40.0), egui::vec2(badge_w, 17.0));
            p.rect_filled(badge_rect, 4.0, BRAND_500.gamma_multiply(0.18));
            p.rect_stroke(badge_rect, 4.0, Stroke::new(1.0, BRAND_500.gamma_multiply(0.35)), egui::StrokeKind::Inside);
            p.text(badge_rect.center(), Align2::CENTER_CENTER, badge, badge_font, BRAND_300);

            // Description.
            p.text(
                pos2(text_left, rect.center().y - 10.0),
                Align2::LEFT_CENTER,
                "Lossless pixel retention. Strips EXIF, IPTC, XMP, C2PA, GPS, and Generative AI metadata.",
                FontId::new(11.5, FontFamily::Proportional),
                MUTED,
            );

            // Format chips.
            let chip_y = rect.center().y + 12.0;
            let mut x = text_left;
            for fmt in ["JPEG", "PNG", "WebP"] {
                let f = FontId::new(9.5, FontFamily::Monospace);
                let w = p.layout_no_wrap(fmt.to_owned(), f.clone(), MUTED).size().x + 14.0;
                let cr = Rect::from_min_size(pos2(x, chip_y), egui::vec2(w, 18.0));
                p.rect_filled(cr, 4.0, BG.gamma_multiply(0.7));
                p.rect_stroke(cr, 4.0, Stroke::new(1.0, BORDER), egui::StrokeKind::Inside);
                p.text(cr.center(), Align2::CENTER_CENTER, fmt, f, MUTED);
                x += w + 6.0;
            }

            // Right column: browse button + shortcut hint.
            let browse_label = "Browse Computer";
            let b_font = FontId::new(11.5, FontFamily::Name(theme::SEMI.into()));
            let bw = p.layout_no_wrap(browse_label.to_owned(), b_font.clone(), TEXT).size().x + 44.0;
            let br = Rect::from_min_size(pos2(right - bw, rect.center().y - 34.0), egui::vec2(bw, 32.0));
            p.rect_filled(br, 10.0, if br.contains(response.hover_pos().unwrap_or_default()) { HOVER } else { PANEL });
            p.rect_stroke(br, 10.0, Stroke::new(1.0, BORDER), egui::StrokeKind::Inside);
            icons::folder(&p, Rect::from_center_size(pos2(br.left() + 14.0, br.center().y), egui::vec2(14.0, 14.0)), BRAND_400);
            p.text(pos2(br.left() + 26.0, br.center().y), Align2::LEFT_CENTER, browse_label, b_font, TEXT);

            p.text(
                pos2(right, rect.center().y + 6.0),
                Align2::RIGHT_CENTER,
                "Ctrl+O or Drag File",
                FontId::new(10.0, FontFamily::Monospace),
                FAINT,
            );
        } else {
            // Compact strip.
            let icon_center = pos2(rect.left() + 34.0, rect.center().y);
            let tile = Rect::from_center_size(icon_center, egui::vec2(40.0, 40.0));
            p.rect_filled(tile, 12.0, lerp_color(BRAND_500.gamma_multiply(0.20), CORAL_500.gamma_multiply(0.10), 0.5));
            p.rect_stroke(tile, 12.0, Stroke::new(1.0, BRAND_500.gamma_multiply(0.30)), egui::StrokeKind::Inside);
            icons::cloud_upload(&p, Rect::from_center_size(icon_center, egui::vec2(22.0, 22.0)), if active { CORAL_400 } else { BRAND_400 });

            let title_font = FontId::new(13.5, FontFamily::Name(theme::SEMI.into()));
            let badge = "FAST · LOSSLESS";
            let badge_font = FontId::new(8.5, FontFamily::Monospace);
            let title_w = p.layout_no_wrap("Drop more images or folders here".to_owned(), title_font.clone(), Color32::WHITE).size().x;
            let badge_w = p.layout_no_wrap(badge.to_owned(), badge_font.clone(), Color32::WHITE).size().x + 14.0;
            p.text(pos2(rect.left() + 66.0, rect.center().y), Align2::LEFT_CENTER, "Drop more images or folders here", title_font, Color32::WHITE);
            let badge_rect = Rect::from_min_size(pos2(rect.left() + 66.0 + title_w + 10.0, rect.center().y - 8.0), egui::vec2(badge_w, 16.0));
            p.rect_filled(badge_rect, 4.0, BRAND_500.gamma_multiply(0.18));
            p.rect_stroke(badge_rect, 4.0, Stroke::new(1.0, BRAND_500.gamma_multiply(0.35)), egui::StrokeKind::Inside);
            p.text(badge_rect.center(), Align2::CENTER_CENTER, badge, badge_font, BRAND_300);

            p.text(
                pos2(rect.right() - 20.0, rect.center().y),
                Align2::RIGHT_CENTER,
                "Browse Computer · Ctrl+O or Drag File",
                FontId::new(10.5, FontFamily::Monospace),
                FAINT,
            );
        }

        if response.clicked() {
            self.browse();
        }
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        let (ok, err) = self.counts();
        ui.horizontal(|ui| {
            // Tabs container.
            egui::Frame::NONE
                .fill(CARD)
                .stroke(Stroke::new(1.0, BORDER))
                .corner_radius(12.0)
                .inner_margin(Margin::same(4))
                .show(ui, |ui| {
                    ui.set_min_height(30.0);
                    ui.horizontal(|ui| {
                        self.tab_button(ui, "All Items", self.items.len(), Tab::All, BRAND_600);
                        self.tab_button(ui, "Cleaned", ok, Tab::Cleaned, EMERALD);
                        self.tab_button(ui, "Issues", err, Tab::Issues, CORAL_500);
                    });
                });

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Sort menu.
                let sort_label = match self.sort {
                    Sort::Added => "Sort".to_owned(),
                    other => other_name(other).to_owned(),
                };
                let font = FontId::new(11.5, FontFamily::Name(theme::SEMI.into()));
                let w = width_of(ui, &sort_label, font.clone()) + 42.0;
                let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, 30.0), Sense::click());
                let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
                let p = ui.painter();
                p.rect_filled(rect, 8.0, if resp.hovered() { HOVER } else { CARD });
                p.rect_stroke(rect, 8.0, Stroke::new(1.0, BORDER), egui::StrokeKind::Inside);
                icons::sort(p, Rect::from_center_size(pos2(rect.left() + 14.0, rect.center().y), egui::vec2(14.0, 14.0)), MUTED);
                p.text(pos2(rect.left() + 26.0, rect.center().y), Align2::LEFT_CENTER, &sort_label, font, TEXT);
                if resp.clicked() {
                    self.sort = self.sort.next();
                }
                if resp.secondary_clicked() {
                    self.sort = Sort::Added;
                }

                ui.add_space(6.0);

                // Detail View toggle.
                let font = FontId::new(11.5, FontFamily::Name(theme::SEMI.into()));
                let w = width_of(ui, "Detail View", font.clone()) + 42.0;
                let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, 30.0), Sense::click());
                let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
                let p = ui.painter();
                p.rect_filled(rect, 8.0, if self.show_details { BRAND_600.gamma_multiply(0.25) } else if resp.hovered() { HOVER } else { CARD });
                p.rect_stroke(rect, 8.0, Stroke::new(1.0, if self.show_details { BRAND_400 } else { BORDER }), egui::StrokeKind::Inside);
                icons::list_check(p, Rect::from_center_size(pos2(rect.left() + 14.0, rect.center().y), egui::vec2(14.0, 14.0)), if self.show_details { BRAND_300 } else { MUTED });
                p.text(pos2(rect.left() + 26.0, rect.center().y), Align2::LEFT_CENTER, "Detail View", font, if self.show_details { Color32::WHITE } else { TEXT });
                if resp.clicked() {
                    self.show_details = !self.show_details;
                }

                ui.add_space(6.0);

                // Search box.
                egui::Frame::NONE
                    .fill(CARD)
                    .stroke(Stroke::new(1.0, BORDER))
                    .corner_radius(8.0)
                    .inner_margin(Margin::symmetric(8, 3))
                    .show(ui, |ui| {
                        ui.set_min_height(24.0);
                        ui.set_width(196.0);
                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            let (r, _) = ui.allocate_exact_size(egui::vec2(12.0, 12.0), Sense::hover());
                            icons::search(ui.painter(), r, FAINT);
                            egui::TextEdit::singleline(&mut self.filter)
                                .hint_text(egui::RichText::new("Filter queue files...").size(11.0).color(DIM))
                                .desired_width(160.0)
                                .frame(egui::Frame::NONE)
                                .show(ui);
                        });
                    });
            });
        });
    }

    fn tab_button(&mut self, ui: &mut egui::Ui, label: &str, count: usize, tab: Tab, count_color: Color32) {
        let selected = self.tab == tab;
        let font = FontId::new(11.5, FontFamily::Name(theme::SEMI.into()));
        let badge_font = FontId::new(9.5, FontFamily::Monospace);
        let count_text = count.to_string();
        let w = width_of(ui, label, font.clone())
            + width_of(ui, &count_text, badge_font.clone())
            + 30.0;
        let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, 30.0), Sense::click());
        let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
        let p = ui.painter();
        if selected {
            p.rect_filled(rect, 8.0, BRAND_600);
        } else if resp.hovered() {
            p.rect_filled(rect, 8.0, PANEL);
        }
        let text_color = if selected { Color32::WHITE } else { MUTED };
        p.text(pos2(rect.left() + 12.0, rect.center().y), Align2::LEFT_CENTER, label, font, text_color);
        // Count badge.
        let bw = p.layout_no_wrap(count_text.clone(), badge_font.clone(), Color32::WHITE).size().x + 10.0;
        let badge = Rect::from_min_size(pos2(rect.right() - bw - 10.0, rect.center().y - 7.0), egui::vec2(bw, 14.0));
        p.rect_filled(badge, 7.0, if selected { Color32::BLACK.gamma_multiply(0.35) } else { count_color.gamma_multiply(0.15) });
        p.text(badge.center(), Align2::CENTER_CENTER, &count_text, badge_font, if selected { Color32::WHITE } else { count_color });
        if resp.clicked() {
            self.tab = tab;
        }
    }
}

fn other_name(sort: Sort) -> &'static str {
    match sort {
        Sort::Added => "Sort: Recent",
        Sort::Name => "Sort: Name",
        Sort::Size => "Sort: Size",
    }
}

fn empty_hint(ui: &mut egui::Ui) {
    ui.vertical_centered(|ui| {
        ui.add_space(6.0);
        let (r, _) = ui.allocate_exact_size(egui::vec2(30.0, 30.0), Sense::hover());
        icons::search_off(ui.painter(), r, DIM);
        ui.add_space(4.0);
        ui.label(semi(13.0, "No files yet", MUTED));
        ui.label(
            RichText::new("Drop AI images or a folder above — everything is cleaned instantly.")
                .size(11.0)
                .color(FAINT),
        );
    });
}

// ---------------------------------------------------------------- result cards

fn result_card(ui: &mut egui::Ui, idx: usize, item: &Item, show_details: bool) -> Option<Action> {
    let mut action = None;
    let name = name_of(&item.source);
    let is_err = item.result.is_err();

    let mut frame = egui::Frame::NONE
        .fill(CARD)
        .corner_radius(16.0)
        .inner_margin(Margin::symmetric(18, 14));
    frame.stroke = match &item.result {
        Ok(_) => Stroke::new(1.0, BORDER),
        Err(_) => Stroke::new(1.0, CORAL_500.gamma_multiply(0.45)),
    };

    frame.show(ui, |ui| {
        ui.horizontal(|ui| {
            // Thumbnail tile with a format band.
            let (tile, _) = ui.allocate_exact_size(egui::vec2(48.0, 48.0), Sense::hover());
            let p = ui.painter();
            let ai = matches!(&item.result, Ok(r) if r.has_ai_data());
            let base = if is_err {
                CORAL_500.gamma_multiply(0.15)
            } else if ai {
                CORAL_500.gamma_multiply(0.18)
            } else {
                BRAND_700.gamma_multiply(0.40)
            };
            p.rect_filled(tile, 12.0, base);
            p.rect_stroke(tile, 12.0, Stroke::new(1.0, if ai { CORAL_500.gamma_multiply(0.4) } else { BRAND_500.gamma_multiply(0.35) }), egui::StrokeKind::Inside);
            let icon_rect = Rect::from_min_max(tile.left_top() + egui::vec2(4.0, 2.0), tile.center() + egui::vec2(0.0, -2.0));
            if is_err {
                icons::alert(p, icon_rect, CORAL_400);
            } else if ai {
                icons::sparkle(p, icon_rect, CORAL_400);
            } else {
                icons::image(p, icon_rect, BRAND_300);
            }
            // Format band.
            if let Ok(r) = &item.result {
                let band = Rect::from_min_max(pos2(tile.left(), tile.bottom() - 14.0), pos2(tile.right(), tile.bottom()));
                p.rect_filled(band, egui::CornerRadius::same(2), if ai { CORAL_600 } else { BRAND_600 });
                p.text(band.center(), Align2::CENTER_CENTER, format_label(r.format), FontId::new(8.0, FontFamily::Monospace), Color32::WHITE);
            }

            ui.add_space(10.0);
            ui.vertical(|ui| {
                ui.horizontal_wrapped(|ui| {
                    // Status dot.
                    let (dot, _) = ui.allocate_exact_size(egui::vec2(12.0, 12.0), Sense::hover());
                    let p = ui.painter();
                    let (dc, glow) = if is_err { (CORAL_500, CORAL_500) } else { (EMERALD, EMERALD) };
                    p.circle_filled(dot.center(), 5.0, dc);
                    p.circle_stroke(dot.center(), 7.5, Stroke::new(2.0, glow.gamma_multiply(0.30)));

                    ui.label(semi(13.5, &name, Color32::WHITE));

                    match &item.result {
                        Ok(r) => {
                            status_chip(ui, icons::check, "0 metadata remaining", EMERALD);
                            for (kind, count) in kind_counts(r) {
                                let (label, color) = kind_chip(kind);
                                status_chip(ui, kind_icon(kind), &format!("{label} · {count}"), color);
                            }
                        }
                        Err(_) => {
                            status_chip(ui, icons::alert, "Failed", CORAL_400);
                        }
                    }
                });

                match &item.result {
                    Ok(r) => metrics_row(ui, r),
                    Err(message) => {
                        ui.add_space(2.0);
                        ui.label(
                            RichText::new(message).size(11.0).color(CORAL_300).family(FontFamily::Monospace),
                        );
                    }
                }
            });
        });

        // Right-side actions, aligned to the top row on a wide window.
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Remove.
                let (x_rect, x_resp) = ui.allocate_exact_size(egui::vec2(28.0, 28.0), Sense::click());
                let x_resp = x_resp.on_hover_cursor(egui::CursorIcon::PointingHand).on_hover_text("Remove item");
                let x_hover = x_resp.hovered();
                if x_hover {
                    ui.painter().rect_filled(x_rect, 7.0, CORAL_500.gamma_multiply(0.12));
                }
                icons::close(ui.painter(), x_rect.shrink(9.0), if x_hover { CORAL_400 } else { FAINT });
                if x_resp.clicked() {
                    action = Some(Action::Remove(idx));
                }

                ui.add_space(4.0);
                // Show in folder.
                if let Ok(r) = &item.result {
                    if show_folder_button(ui, "Show in folder").clicked() {
                        action = Some(Action::Reveal(r.output.clone()));
                    }
                    ui.add_space(4.0);
                    // Inspection Report toggle.
                    let open = show_details || item.drawer_open;
                    if inspection_button(ui, "Inspection Report", open).clicked() {
                        action = Some(Action::ToggleDrawer(idx));
                    }
                    ui.add_space(4.0);
                    // Output chip.
                    let out_name = name_of(&r.output);
                    let font = FontId::new(11.0, FontFamily::Monospace);
                    let w = width_of(ui, &out_name, font.clone()) + 36.0;
                    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, 30.0), Sense::click());
                    let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand)
                        .on_hover_text(format!("Open: {}", r.output.display()));
                    ui.painter().rect_filled(rect, 8.0, PANEL);
                    ui.painter().rect_stroke(rect, 8.0, Stroke::new(1.0, BORDER), egui::StrokeKind::Inside);
                    icons::download(ui.painter(), Rect::from_center_size(pos2(rect.left() + 13.0, rect.center().y), egui::vec2(14.0, 14.0)), BRAND_400);
                    ui.painter().text(pos2(rect.left() + 24.0, rect.center().y), Align2::LEFT_CENTER, &out_name, font, TEXT);
                    if resp.clicked() {
                        action = Some(Action::Reveal(r.output.clone()));
                    }
                }
            });
        });

        // Purged elements drawer.
        if let Ok(r) = &item.result {
            let open = show_details || item.drawer_open;
            if open && !r.removals.is_empty() {
                ui.add_space(12.0);
                let (line, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 1.0), Sense::hover());
                ui.painter().line_segment([pos2(line.left(), line.center().y), pos2(line.right(), line.center().y)], Stroke::new(1.0, BORDER_SUBTLE));
                ui.add_space(10.0);
                ui.horizontal_wrapped(|ui| {
                    ui.label(mono(10.0, "PURGED ELEMENTS:", FAINT));
                    ui.add_space(2.0);
                    let max = 8;
                    for (i, removal) in r.removals.iter().take(max).enumerate() {
                        purged_chip(ui, removal);
                        if i == 0 {
                            ui.add_space(4.0);
                        }
                    }
                    if r.removals.len() > max {
                        ui.label(mono(10.0, format!("+ {} more (see report)", r.removals.len() - max), DIM));
                    }
                });
            }
        }
    });

    action
}

// NOTE: Action needs a ToggleDrawer variant; defined here to keep the enum near use.
impl App {
    fn handle(&mut self, action: Action) {
        match action {
            Action::Remove(idx) => {
                if idx < self.items.len() {
                    self.items.remove(idx);
                }
            }
            Action::Reveal(path) => reveal_in_explorer(&path),
            Action::Export => self.export_all(),
            Action::ToggleDrawer(idx) => {
                if let Some(item) = self.items.get_mut(idx) {
                    item.drawer_open = !item.drawer_open;
                }
            }
        }
    }
}

fn format_label(f: Format) -> &'static str {
    match f {
        Format::Png => "PNG",
        Format::Jpeg => "JPEG",
        Format::WebP => "WEBP",
    }
}

/// Per-kind counts in stable display order.
fn kind_counts(r: &Report) -> Vec<(Kind, usize)> {
    const ORDER: [Kind; 9] = [
        Kind::AiData, Kind::C2pa, Kind::Exif, Kind::Xmp, Kind::Iptc, Kind::Comment,
        Kind::Timestamp, Kind::Thumbnail, Kind::Other,
    ];
    ORDER
        .iter()
        .filter_map(|k| {
            let n = r.removals.iter().filter(|x| x.kind == *k).count();
            (n > 0).then_some((*k, n))
        })
        .collect()
}

fn kind_chip(kind: Kind) -> (&'static str, Color32) {
    match kind {
        Kind::AiData => ("AI Prompts & Seeds Purged", CORAL_400),
        Kind::C2pa => ("C2PA credentials stripped", BRAND_400),
        Kind::Exif => ("EXIF & GPS stripped", BRAND_400),
        Kind::Xmp => ("XMP stripped", BRAND_400),
        Kind::Iptc => ("IPTC stripped", BRAND_400),
        Kind::Comment => ("Comments stripped", BRAND_400),
        Kind::Timestamp => ("Timestamps stripped", BRAND_400),
        Kind::Thumbnail => ("Thumbnails stripped", BRAND_400),
        Kind::Other => ("Hidden data stripped", BRAND_400),
    }
}

fn kind_icon(kind: Kind) -> fn(&Painter, Rect, Color32) {
    match kind {
        Kind::AiData => icons::sparkle,
        Kind::C2pa => icons::fingerprint,
        Kind::Exif => icons::camera,
        Kind::Xmp => icons::tags,
        Kind::Iptc => icons::id_badge,
        Kind::Comment => icons::quote,
        Kind::Timestamp => icons::clock,
        Kind::Thumbnail => icons::image,
        Kind::Other => icons::dot,
    }
}

fn status_chip(ui: &mut egui::Ui, icon: fn(&Painter, Rect, Color32), text: &str, color: Color32) {
    let font = FontId::new(10.0, FontFamily::Monospace);
    let icon_w = 10.0;
    let w = width_of(ui, text, font.clone()) + icon_w + 22.0;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(w, 18.0), Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, 9.0, color.gamma_multiply(0.10));
    p.rect_stroke(rect, 9.0, Stroke::new(1.0, color.gamma_multiply(0.30)), egui::StrokeKind::Inside);
    icon(p, Rect::from_center_size(pos2(rect.left() + 11.0, rect.center().y), egui::vec2(9.0, 9.0)), color);
    p.text(pos2(rect.left() + 19.0, rect.center().y), Align2::LEFT_CENTER, text, font, color);
}

fn metrics_row(ui: &mut egui::Ui, r: &Report) {
    ui.horizontal(|ui| {
        ui.add_space(22.0);
        let saved = r.saved();
        ui.label(mono(11.0, human_size(r.original), MUTED));
        ui.label(RichText::new("→").size(11.0).color(FAINT).family(FontFamily::Monospace));
        ui.label(mono(11.0, human_size(r.cleaned), TEXT));
        if saved > 0 {
            let pct = if r.original > 0 {
                format!(" ({:.1}%)", saved as f64 / r.original as f64 * 100.0)
            } else {
                String::new()
            };
            saved_chip(ui, &format!("-{}{} saved", human_size(saved), pct));
        } else {
            ui.label(mono(10.5, "nothing to purge", DIM));
        }
        ui.label(RichText::new("•").size(11.0).color(FAINT));
        ui.label(
            RichText::new("Pixel-identical · Lossless").size(11.0).color(MUTED),
        );
    });
}

fn saved_chip(ui: &mut egui::Ui, text: &str) {
    let font = FontId::new(10.5, FontFamily::Monospace);
    let w = width_of(ui, text, font.clone()) + 16.0;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(w, 20.0), Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, 5.0, EMERALD_DIM.gamma_multiply(0.35));
    p.rect_stroke(rect, 5.0, Stroke::new(1.0, EMERALD.gamma_multiply(0.30)), egui::StrokeKind::Inside);
    p.text(rect.center(), Align2::CENTER_CENTER, text, font, EMERALD);
}

fn purged_chip(ui: &mut egui::Ui, removal: &Removal) {
    let text = format!("{} · {} B", removal.name, removal.size);
    let font = FontId::new(10.5, FontFamily::Monospace);
    let color = kind_color(removal.kind);
    let icon_w = 12.0;
    let w = width_of(ui, &text, font.clone()) + icon_w + 24.0;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(w, 22.0), Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, 6.0, PANEL.gamma_multiply(0.6));
    p.rect_stroke(rect, 6.0, Stroke::new(1.0, BORDER), egui::StrokeKind::Inside);
    kind_icon(removal.kind)(p, Rect::from_center_size(pos2(rect.left() + 13.0, rect.center().y), egui::vec2(11.0, 11.0)), color);
    p.text(pos2(rect.left() + 23.0, rect.center().y), Align2::LEFT_CENTER, text, font, Color32::from_rgb(0xCB, 0xD5, 0xE1));
}

fn kind_color(kind: Kind) -> Color32 {
    match kind {
        Kind::AiData => CORAL_400,
        Kind::C2pa | Kind::Exif | Kind::Xmp | Kind::Iptc => BRAND_400,
        Kind::Comment | Kind::Timestamp | Kind::Thumbnail => BRAND_300,
        Kind::Other => FAINT,
    }
}

/// "Show in folder" button: violet-tinted, prominent.
fn show_folder_button(ui: &mut egui::Ui, label: &str) -> egui::Response {
    let font = FontId::new(11.5, FontFamily::Name(theme::SEMI.into()));
    let text_w = width_of(ui, label, font.clone());
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(text_w + 42.0, 30.0), Sense::click());
    let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
    let hover = resp.hovered();
    let p = ui.painter();
    p.rect_filled(rect, 8.0, if hover { BRAND_600.gamma_multiply(0.30) } else { BRAND_600.gamma_multiply(0.18) });
    p.rect_stroke(rect, 8.0, Stroke::new(1.0, BRAND_500.gamma_multiply(0.45)), egui::StrokeKind::Inside);
    icons::folder(p, Rect::from_center_size(pos2(rect.left() + 14.0, rect.center().y), egui::vec2(14.0, 14.0)), BRAND_400);
    p.text(pos2(rect.left() + 26.0, rect.center().y), Align2::LEFT_CENTER, label, font, if hover { Color32::WHITE } else { BRAND_300 });
    resp
}

/// "Inspection Report" button; `open` shows the active state.
fn inspection_button(ui: &mut egui::Ui, label: &str, open: bool) -> egui::Response {
    let font = FontId::new(11.5, FontFamily::Name(theme::SEMI.into()));
    let text_w = width_of(ui, label, font.clone());
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(text_w + 42.0, 30.0), Sense::click());
    let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
    let hover = resp.hovered();
    let p = ui.painter();
    p.rect_filled(rect, 8.0, if open { BRAND_600.gamma_multiply(0.22) } else if hover { HOVER } else { PANEL });
    p.rect_stroke(rect, 8.0, Stroke::new(1.0, if open { BRAND_400 } else { BORDER }), egui::StrokeKind::Inside);
    icons::tags(p, Rect::from_center_size(pos2(rect.left() + 14.0, rect.center().y), egui::vec2(14.0, 14.0)), if open { BRAND_300 } else { BRAND_400 });
    p.text(pos2(rect.left() + 26.0, rect.center().y), Align2::LEFT_CENTER, label, font, if open || hover { Color32::WHITE } else { TEXT });
    resp
}

// ---------------------------------------------------------------- geometry + misc

fn human_size(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    let b = bytes as f64;
    if b < KB {
        format!("{bytes} B")
    } else if b < KB * KB {
        format!("{:.1} KB", b / KB)
    } else {
        format!("{:.2} MB", b / (KB * KB))
    }
}

/// Rounded-rectangle outline as a closed polyline, so it can be dashed.
fn rounded_outline(rect: Rect, r: f32) -> Vec<egui::Pos2> {
    let corners = [
        (rect.right() - r, rect.top() + r, -90.0_f32),
        (rect.right() - r, rect.bottom() - r, 0.0),
        (rect.left() + r, rect.bottom() - r, 90.0),
        (rect.left() + r, rect.top() + r, 180.0),
    ];
    let mut pts = Vec::new();
    for (cx, cy, start) in corners {
        for k in 0..=8 {
            let a = (start + 90.0 * k as f32 / 8.0).to_radians();
            pts.push(egui::pos2(cx + r * a.cos(), cy + r * a.sin()));
        }
    }
    pts.push(pts[0]);
    pts
}

fn dashed(painter: &Painter, pts: &[egui::Pos2], stroke: Stroke, dash: f32, gap: f32) {
    let mut drawing = true;
    let mut left = dash;
    for seg in pts.windows(2) {
        let (a, b) = (seg[0], seg[1]);
        let len = (b - a).length();
        if len <= f32::EPSILON {
            continue;
        }
        let dir = (b - a) / len;
        let mut t = 0.0;
        while t < len {
            let step = left.min(len - t);
            if drawing {
                painter.line_segment([a + dir * t, a + dir * (t + step)], stroke);
            }
            t += step;
            left -= step;
            if left <= f32::EPSILON {
                drawing = !drawing;
                left = if drawing { dash } else { gap };
            }
        }
    }
}

fn reveal_in_explorer(path: &Path) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let _ = std::process::Command::new("explorer")
            .raw_arg(format!("/select,\"{}\"", path.display()))
            .spawn();
    }
    #[cfg(not(windows))]
    {
        if let Some(dir) = path.parent() {
            let _ = std::process::Command::new("xdg-open").arg(dir).spawn();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_are_human_readable() {
        assert_eq!(human_size(512), "512 B");
        assert_eq!(human_size(2048), "2.0 KB");
        assert_eq!(human_size(3 * 1024 * 1024), "3.00 MB");
    }

    #[test]
    fn outline_is_closed() {
        let r = Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(100.0, 60.0));
        let pts = rounded_outline(r, 12.0);
        assert_eq!(pts.first(), pts.last());
    }

    #[test]
    fn sort_cycles() {
        assert_eq!(Sort::Added.next(), Sort::Name);
        assert_eq!(Sort::Name.next(), Sort::Size);
        assert_eq!(Sort::Size.next(), Sort::Added);
    }

    #[test]
    fn kind_counts_are_ordered() {
        let r = Report {
            output: PathBuf::from("a"),
            format: Format::Png,
            original: 10,
            cleaned: 5,
            removals: vec![
                Removal { kind: Kind::Exif, name: "eXIf".into(), size: 3 },
                Removal { kind: Kind::AiData, name: "tEXt".into(), size: 2 },
            ],
        };
        let counts = kind_counts(&r);
        assert_eq!(counts[0].0, Kind::AiData);
        assert_eq!(counts[1].0, Kind::Exif);
    }
}
