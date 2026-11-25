//! Vector icons drawn with the egui painter. Each icon is designed on a unit grid (0..1)
//! and mapped into the rect it is given, so they stay crisp at any size.

use egui::{pos2, Color32, Painter, Pos2, Rect, Shape, Stroke};

/// Maps a unit-grid point into `rect`.
fn p(rect: Rect, x: f32, y: f32) -> Pos2 {
    pos2(rect.left() + x * rect.width(), rect.top() + y * rect.height())
}

fn stroke(rect: Rect, color: Color32, weight: f32) -> Stroke {
    Stroke::new(rect.width() * weight, color)
}

fn polyline(painter: &Painter, rect: Rect, pts: &[(f32, f32)], color: Color32, w: f32) {
    let pts: Vec<Pos2> = pts.iter().map(|&(x, y)| p(rect, x, y)).collect();
    painter.add(Shape::line(pts, stroke(rect, color, w)));
}

fn closed(painter: &Painter, rect: Rect, pts: &[(f32, f32)], color: Color32, w: f32) {
    let pts: Vec<Pos2> = pts.iter().map(|&(x, y)| p(rect, x, y)).collect();
    painter.add(Shape::closed_line(pts, stroke(rect, color, w)));
}

/// Picture frame with mountains and a sun. Used for image rows.
pub fn image(painter: &Painter, rect: Rect, color: Color32) {
    let w = 0.085;
    let frame = Rect::from_min_max(p(rect, 0.08, 0.18), p(rect, 0.92, 0.82));
    painter.rect_stroke(
        frame,
        egui::CornerRadius::same((rect.width() * 0.1) as u8),
        stroke(rect, color, w),
        egui::StrokeKind::Middle,
    );
    polyline(painter, rect, &[(0.2, 0.7), (0.4, 0.46), (0.55, 0.6), (0.68, 0.48), (0.82, 0.7)], color, w * 0.85);
    painter.circle_filled(p(rect, 0.67, 0.33), rect.width() * 0.07, color);
}

/// Cloud with an upward arrow. Used for the drop zone.
pub fn cloud_upload(painter: &Painter, rect: Rect, color: Color32) {
    let w = 0.075;
    let base = [(0.12, 0.72), (0.12, 0.56), (0.28, 0.56), (0.3, 0.4), (0.48, 0.3), (0.7, 0.34), (0.86, 0.5), (0.86, 0.72)];
    closed(painter, rect, &base, color, w);
    polyline(painter, rect, &[(0.5, 0.86), (0.5, 0.52)], color, w);
    polyline(painter, rect, &[(0.36, 0.64), (0.5, 0.5), (0.64, 0.64)], color, w);
}

pub fn close(painter: &Painter, rect: Rect, color: Color32) {
    let w = 0.1;
    polyline(painter, rect, &[(0.26, 0.26), (0.74, 0.74)], color, w);
    polyline(painter, rect, &[(0.74, 0.26), (0.26, 0.74)], color, w);
}

pub fn check(painter: &Painter, rect: Rect, color: Color32) {
    polyline(painter, rect, &[(0.2, 0.52), (0.42, 0.74), (0.8, 0.28)], color, 0.13);
}

pub fn alert(painter: &Painter, rect: Rect, color: Color32) {
    let w = 0.09;
    closed(painter, rect, &[(0.5, 0.14), (0.88, 0.82), (0.12, 0.82)], color, w);
    polyline(painter, rect, &[(0.5, 0.42), (0.5, 0.6)], color, w);
    painter.circle_filled(p(rect, 0.5, 0.72), rect.width() * 0.055, color);
}

pub fn trash(painter: &Painter, rect: Rect, color: Color32) {
    let w = 0.08;
    polyline(painter, rect, &[(0.18, 0.3), (0.82, 0.3)], color, w);
    polyline(painter, rect, &[(0.4, 0.3), (0.4, 0.16), (0.6, 0.16), (0.6, 0.3)], color, w);
    polyline(painter, rect, &[(0.28, 0.3), (0.34, 0.86), (0.66, 0.86), (0.72, 0.3)], color, w);
}

pub fn folder(painter: &Painter, rect: Rect, color: Color32) {
    closed(
        painter,
        rect,
        &[(0.1, 0.26), (0.38, 0.26), (0.46, 0.36), (0.9, 0.36), (0.9, 0.8), (0.1, 0.8)],
        color,
        0.085,
    );
}

/// Shield with a check. Used for the privacy notice and profile chip.
pub fn shield(painter: &Painter, rect: Rect, color: Color32) {
    closed(
        painter,
        rect,
        &[(0.5, 0.1), (0.86, 0.24), (0.84, 0.54), (0.5, 0.9), (0.16, 0.54), (0.14, 0.24)],
        color,
        0.09,
    );
    polyline(painter, rect, &[(0.34, 0.5), (0.46, 0.62), (0.68, 0.38)], color, 0.09);
}

/// Down-then-right arrow (fa "arrow-turn-down"), used for the output file chip.
pub fn download(painter: &Painter, rect: Rect, color: Color32) {
    polyline(painter, rect, &[(0.72, 0.2), (0.72, 0.58)], color, 0.1);
    polyline(painter, rect, &[(0.48, 0.38), (0.72, 0.62), (0.96, 0.38)], color, 0.1);
    polyline(painter, rect, &[(0.72, 0.6), (0.56, 0.6), (0.14, 0.6), (0.14, 0.86)], color, 0.1);
}

pub fn plus(painter: &Painter, rect: Rect, color: Color32) {
    polyline(painter, rect, &[(0.5, 0.2), (0.5, 0.8)], color, 0.11);
    polyline(painter, rect, &[(0.2, 0.5), (0.8, 0.5)], color, 0.11);
}

pub fn sliders(painter: &Painter, rect: Rect, color: Color32) {
    let w = 0.08;
    for (y, kx) in [(0.24, 0.62), (0.5, 0.34), (0.76, 0.7)] {
        polyline(painter, rect, &[(0.12, y), (0.88, y)], color, w);
        painter.circle_filled(p(rect, kx, y), rect.width() * 0.085, color);
    }
}

pub fn search(painter: &Painter, rect: Rect, color: Color32) {
    painter.circle_stroke(p(rect, 0.42, 0.42), rect.width() * 0.26, stroke(rect, color, 0.1));
    polyline(painter, rect, &[(0.62, 0.62), (0.86, 0.86)], color, 0.1);
}

/// fa-list-check style: lines with small check marks.
pub fn list_check(painter: &Painter, rect: Rect, color: Color32) {
    let w = 0.085;
    polyline(painter, rect, &[(0.1, 0.3), (0.24, 0.44), (0.44, 0.2)], color, w * 1.2);
    polyline(painter, rect, &[(0.58, 0.32), (0.9, 0.32)], color, w);
    polyline(painter, rect, &[(0.1, 0.78), (0.24, 0.92), (0.44, 0.68)], color, w * 1.2);
    polyline(painter, rect, &[(0.58, 0.8), (0.9, 0.8)], color, w);
}

/// fa-arrow-down-wide-short: descending bars with an arrow.
pub fn sort(painter: &Painter, rect: Rect, color: Color32) {
    let w = 0.085;
    polyline(painter, rect, &[(0.2, 0.14), (0.2, 0.82)], color, w);
    polyline(painter, rect, &[(0.06, 0.66), (0.2, 0.84), (0.34, 0.66)], color, w);
    polyline(painter, rect, &[(0.5, 0.32), (0.92, 0.32)], color, w);
    polyline(painter, rect, &[(0.5, 0.54), (0.82, 0.54)], color, w);
    polyline(painter, rect, &[(0.5, 0.76), (0.7, 0.76)], color, w);
}

/// fa-tags: two overlapping tags.
pub fn tags(painter: &Painter, rect: Rect, color: Color32) {
    let w = 0.075;
    closed(painter, rect, &[(0.5, 0.12), (0.86, 0.12), (0.86, 0.48), (0.52, 0.84), (0.2, 0.52)], color, w);
    painter.circle_filled(p(rect, 0.7, 0.3), rect.width() * 0.06, color);
    polyline(painter, rect, &[(0.14, 0.26), (0.14, 0.52), (0.36, 0.76)], color, w * 0.8);
}

/// fa-file-zipper for the Export All button.
pub fn file_zipper(painter: &Painter, rect: Rect, color: Color32) {
    let w = 0.08;
    closed(painter, rect, &[(0.2, 0.08), (0.66, 0.08), (0.84, 0.28), (0.84, 0.9), (0.2, 0.9)], color, w);
    polyline(painter, rect, &[(0.64, 0.1), (0.64, 0.3), (0.84, 0.3)], color, w * 0.8);
    for y in [0.34, 0.46, 0.58] {
        polyline(painter, rect, &[(0.42, y), (0.42, y + 0.08)], color, w * 1.4);
    }
    polyline(painter, rect, &[(0.36, 0.72), (0.48, 0.72), (0.48, 0.86), (0.36, 0.86)], color, w * 0.9);
}

/// Camera. EXIF chip icon.
pub fn camera(painter: &Painter, rect: Rect, color: Color32) {
    let w = 0.08;
    let body = Rect::from_min_max(p(rect, 0.08, 0.26), p(rect, 0.92, 0.8));
    painter.rect_stroke(body, (rect.width() * 0.08) as u8, stroke(rect, color, w), egui::StrokeKind::Middle);
    polyline(painter, rect, &[(0.36, 0.26), (0.4, 0.16), (0.6, 0.16), (0.64, 0.26)], color, w);
    painter.circle_stroke(p(rect, 0.5, 0.53), rect.width() * 0.15, stroke(rect, color, w * 0.95));
}

/// Fingerprint arcs. C2PA chip icon.
pub fn fingerprint(painter: &Painter, rect: Rect, color: Color32) {
    let w = 0.075;
    let c = p(rect, 0.5, 0.5);
    // Concentric partial rings approximated by stroked circles, clipped by the chip.
    painter.circle_stroke(c, rect.width() * 0.40, stroke(rect, color, w));
    painter.circle_stroke(c, rect.width() * 0.26, stroke(rect, color, w * 0.8));
    polyline(painter, rect, &[(0.5, 0.42), (0.5, 0.86)], color, w);
    polyline(painter, rect, &[(0.34, 0.62), (0.34, 0.78)], color, w);
    polyline(painter, rect, &[(0.66, 0.62), (0.66, 0.78)], color, w);
}

/// ID badge. IPTC chip icon.
pub fn id_badge(painter: &Painter, rect: Rect, color: Color32) {
    let w = 0.08;
    let body = Rect::from_min_max(p(rect, 0.12, 0.18), p(rect, 0.88, 0.86));
    painter.rect_stroke(body, (rect.width() * 0.08) as u8, stroke(rect, color, w), egui::StrokeKind::Middle);
    polyline(painter, rect, &[(0.4, 0.1), (0.6, 0.1), (0.6, 0.2), (0.4, 0.2)], color, w * 0.9);
    painter.circle_stroke(p(rect, 0.36, 0.46), rect.width() * 0.09, stroke(rect, color, w * 0.9));
    polyline(painter, rect, &[(0.26, 0.74), (0.26, 0.68), (0.46, 0.68), (0.46, 0.74)], color, w * 0.85);
    polyline(painter, rect, &[(0.58, 0.46), (0.78, 0.46)], color, w * 0.85);
    polyline(painter, rect, &[(0.58, 0.6), (0.78, 0.6)], color, w * 0.85);
}

/// Quote mark. Comment chip icon.
pub fn quote(painter: &Painter, rect: Rect, color: Color32) {
    let w = 0.1;
    for x0 in [0.14, 0.54] {
        polyline(painter, rect, &[(x0 + 0.28, 0.2), (x0, 0.2), (x0, 0.56)], color, w);
        polyline(painter, rect, &[(x0, 0.56), (x0, 0.8), (x0 + 0.2, 0.8)], color, w * 0.7);
    }
}

/// Clock. Timestamp chip icon.
pub fn clock(painter: &Painter, rect: Rect, color: Color32) {
    let w = 0.085;
    painter.circle_stroke(p(rect, 0.5, 0.5), rect.width() * 0.38, stroke(rect, color, w));
    polyline(painter, rect, &[(0.5, 0.28), (0.5, 0.52), (0.68, 0.6)], color, w);
}

/// Four-point sparkle. AI generation data chip icon.
pub fn sparkle(painter: &Painter, rect: Rect, color: Color32) {
    let w = 0.07;
    polyline(painter, rect, &[(0.5, 0.1), (0.5, 0.62)], color, w);
    polyline(painter, rect, &[(0.24, 0.36), (0.76, 0.36)], color, w);
    polyline(painter, rect, &[(0.72, 0.66), (0.72, 0.92)], color, w);
    polyline(painter, rect, &[(0.58, 0.79), (0.86, 0.79)], color, w);
}

/// Generic small dot for unknown purged elements.
pub fn dot(painter: &Painter, rect: Rect, color: Color32) {
    painter.circle_filled(p(rect, 0.5, 0.5), rect.width() * 0.18, color);
}

/// Magnifier with a plus — "nothing found" empty state.
pub fn search_off(painter: &Painter, rect: Rect, color: Color32) {
    search(painter, rect, color);
    polyline(painter, rect, &[(0.1, 0.86), (0.9, 0.1)], color, 0.07);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_grid_maps_into_rect() {
        let r = Rect::from_min_size(pos2(10.0, 20.0), egui::vec2(40.0, 40.0));
        assert_eq!(p(r, 0.0, 0.0), pos2(10.0, 20.0));
        assert_eq!(p(r, 1.0, 1.0), pos2(50.0, 60.0));
    }
}
