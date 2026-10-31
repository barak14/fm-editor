//! Shared asset-free football pitch and kit artwork. Coordinates attack upwards.
use eframe::egui::{self, Color32, Painter, Pos2, Rect, Stroke, StrokeKind, Vec2};

pub(crate) const OUTFIELD: Color32 = Color32::from_rgb(48, 104, 190);
pub(crate) const KEEPER: Color32 = Color32::from_rgb(235, 179, 55);
pub(crate) const UNFILLED: Color32 = Color32::from_rgb(87, 99, 103);

/// Paint a complete pitch, returning the playable rectangle (goals lie outside it).
pub(crate) fn paint(painter: &Painter, rect: Rect) -> Rect {
    painter.rect_filled(rect, 6.0, Color32::from_rgb(26, 80, 49));
    let field = rect.shrink(14.0);
    for stripe in 0..10 {
        let band = Rect::from_min_max(
            egui::pos2(
                field.left(),
                field.top() + field.height() * stripe as f32 / 10.0,
            ),
            egui::pos2(
                field.right(),
                field.top() + field.height() * (stripe + 1) as f32 / 10.0,
            ),
        );
        painter.rect_filled(
            band,
            0.0,
            if stripe % 2 == 0 {
                Color32::from_rgb(38, 111, 65)
            } else {
                Color32::from_rgb(32, 99, 58)
            },
        );
    }
    let line = Stroke::new(1.2_f32, Color32::from_rgb(208, 230, 208));
    painter.rect_stroke(field, 0.0, line, StrokeKind::Inside);
    painter.line_segment([point(field, 0.0, 0.5), point(field, 1.0, 0.5)], line);
    let radius = field.width().min(field.height()) * 0.13;
    painter.circle_stroke(field.center(), radius, line);
    painter.circle_filled(field.center(), 2.0, line.color);
    for bottom in [false, true] {
        let y = |v: f32| if bottom { 1.0 - v } else { v };
        for (half_width, depth) in [(0.30, 0.16), (0.14, 0.055)] {
            let a = point(field, 0.5 - half_width, y(0.0));
            let b = point(field, 0.5 + half_width, y(depth));
            painter.rect_stroke(Rect::from_two_pos(a, b), 0.0, line, StrokeKind::Inside);
        }
        painter.circle_filled(point(field, 0.5, y(0.11)), 2.0, line.color);
        let goal = Rect::from_two_pos(point(field, 0.44, y(0.0)), point(field, 0.56, y(-0.018)));
        painter.rect_stroke(goal, 0.0, line, StrokeKind::Inside);
    }
    field
}

pub(crate) fn point(field: Rect, x: f32, y: f32) -> Pos2 {
    field.min + Vec2::new(field.width() * x, field.height() * y)
}

/// A vector shirt with a high-contrast abbreviation; no font icons or assets.
pub(crate) fn shirt(painter: &Painter, rect: Rect, label: &str, color: Color32) {
    let p = |x, y| point(rect, x, y);
    // Convex panels avoid triangulation assumptions for the concave shirt outline.
    let stroke = Stroke::new(1.0_f32, Color32::from_rgb(225, 235, 240));
    painter.add(egui::Shape::convex_polygon(
        vec![p(0.24, 0.08), p(0.76, 0.08), p(0.76, 1.0), p(0.24, 1.0)],
        color,
        Stroke::NONE,
    ));
    for points in [
        vec![p(0.24, 0.08), p(0.0, 0.29), p(0.12, 0.53), p(0.3, 0.4)],
        vec![p(0.76, 0.08), p(1.0, 0.29), p(0.88, 0.53), p(0.7, 0.4)],
    ] {
        painter.add(egui::Shape::convex_polygon(points, color, Stroke::NONE));
    }
    painter.add(egui::Shape::closed_line(
        vec![
            p(0.24, 0.08),
            p(0.0, 0.29),
            p(0.12, 0.53),
            p(0.24, 0.45),
            p(0.24, 1.0),
            p(0.76, 1.0),
            p(0.76, 0.45),
            p(0.88, 0.53),
            p(1.0, 0.29),
            p(0.76, 0.08),
            p(0.59, 0.08),
            p(0.5, 0.22),
            p(0.41, 0.08),
        ],
        stroke,
    ));
    // Dark backing makes the text readable on both yellow and blue kits.
    let text_rect = Rect::from_center_size(p(0.5, 0.62), Vec2::new(rect.width() * 0.86, 16.0));
    painter.rect_filled(text_rect, 3.0, Color32::from_rgb(19, 36, 48));
    painter.text(
        text_rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        egui::FontId::proportional(11.0),
        Color32::WHITE,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pitch_and_shirts_render_inside_desktop_and_narrow_frames() {
        for width in [260.0, 1000.0] {
            let ctx = egui::Context::default();
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(width, 700.0))),
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        let (rect, _) = ui.allocate_exact_size(
                            Vec2::new(ui.available_width(), 600.0),
                            egui::Sense::hover(),
                        );
                        let field = paint(ui.painter(), rect);
                        for (x, y, label, color) in
                            [(0.15, 0.25, "AML", OUTFIELD), (0.5, 0.9, "GK", KEEPER)]
                        {
                            let kit =
                                Rect::from_center_size(point(field, x, y), Vec2::new(44.0, 30.0));
                            assert!(field.contains_rect(kit));
                            shirt(ui.painter(), kit, label, color);
                        }
                    });
                },
            );
            assert!(output.shapes.len() > 30);
        }
    }
}
