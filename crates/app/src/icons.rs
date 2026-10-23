use eframe::egui::{self, Color32, Pos2, Rect, Stroke, Vec2};

const GOLD: Color32 = Color32::from_rgb(232, 196, 122);
const INK: Color32 = Color32::from_rgb(236, 228, 210);

pub enum Icon {
    Games,
    Shuffle,
    Undo,
    Redo,
    Auto,
    Themes,
    Restart,
}

pub fn token(ui: &mut egui::Ui, icon: Icon, tip: &str, enabled: bool, active: bool) -> bool {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(54.0), egui::Sense::click());
    let painter = ui.painter();
    let center = rect.center();
    let fill = if !enabled {
        Color32::from_rgba_unmultiplied(18, 22, 20, 120)
    } else if active {
        Color32::from_rgb(92, 64, 24)
    } else if response.hovered() {
        Color32::from_rgba_unmultiplied(28, 40, 32, 230)
    } else {
        Color32::from_rgba_unmultiplied(16, 24, 20, 210)
    };
    painter.circle_filled(center, 24.0, fill);
    painter.circle_stroke(
        center,
        24.0,
        Stroke::new(1.6_f32, if active { GOLD } else { Color32::from_white_alpha(150) }),
    );
    let tint = if enabled { INK } else { Color32::from_white_alpha(80) };
    draw_icon(&painter, center, icon, tint);
    response.clone().on_hover_text(tip);
    response.clicked() && enabled
}

fn draw_icon(painter: &egui::Painter, center: Pos2, icon: Icon, tint: Color32) {
    let stroke = Stroke::new(2.0_f32, tint);
    match icon {
        Icon::Undo => arrow(painter, center, -1.0, stroke),
        Icon::Redo => arrow(painter, center, 1.0, stroke),
        Icon::Shuffle => {
            arc_arrow(painter, center, true, stroke);
            arc_arrow(painter, center, false, stroke);
        }
        Icon::Games => {
            for (index, offset) in [Vec2::new(-8.0, -6.0), Vec2::new(2.0, -2.0), Vec2::new(8.0, 6.0)]
                .into_iter()
                .enumerate()
            {
                let rect = Rect::from_center_size(center + offset, Vec2::new(14.0, 18.0));
                painter.rect_stroke(rect, 2.0, stroke, egui::StrokeKind::Inside);
                if index == 2 {
                    painter.circle_filled(rect.center(), 2.2, tint);
                }
            }
        }
        Icon::Auto => {
            let tip = center + Vec2::new(8.0, 0.0);
            let top = center + Vec2::new(-6.0, -8.0);
            let bottom = center + Vec2::new(-6.0, 8.0);
            painter.add(egui::Shape::convex_polygon(
                vec![tip, top, bottom],
                tint,
                Stroke::NONE,
            ));
        }
        Icon::Restart => {
            painter.circle_stroke(center, 8.0, stroke);
            let tip = center + Vec2::new(6.0, -6.0);
            painter.line_segment([tip, tip + Vec2::new(0.0, 7.0)], stroke);
            painter.line_segment([tip, tip + Vec2::new(-6.0, 1.0)], stroke);
        }
        Icon::Themes => {
            painter.circle_stroke(center + Vec2::new(-6.0, -4.0), 7.0, stroke);
            painter.rect_stroke(
                Rect::from_center_size(center + Vec2::new(5.0, 4.0), Vec2::new(16.0, 12.0)),
                2.0,
                stroke,
                egui::StrokeKind::Inside,
            );
        }
    }
}

fn arrow(painter: &egui::Painter, center: Pos2, dir: f32, stroke: Stroke) {
    let tip = center + Vec2::new(dir * 11.0, 0.0);
    let tail = center + Vec2::new(-dir * 10.0, 0.0);
    painter.line_segment([tail, tip], stroke);
    painter.line_segment([tip, tip + Vec2::new(-dir * 7.0, -6.0)], stroke);
    painter.line_segment([tip, tip + Vec2::new(-dir * 7.0, 6.0)], stroke);
}

fn arc_arrow(painter: &egui::Painter, center: Pos2, upper: bool, stroke: Stroke) {
    let sign = if upper { -1.0 } else { 1.0 };
    let mut points = Vec::new();
    for step in 0..8 {
        let angle = step as f32 / 7.0 * std::f32::consts::PI;
        let x = angle.cos() * 10.0 * if upper { -1.0 } else { 1.0 };
        let y = sign * (4.0 + angle.sin() * 7.0);
        points.push(center + Vec2::new(x, y));
    }
    for pair in points.windows(2) {
        painter.line_segment([pair[0], pair[1]], stroke);
    }
    let tip = *points.last().unwrap();
    painter.line_segment([tip, tip + Vec2::new(if upper { 5.0 } else { -5.0 }, sign * -4.0)], stroke);
}
