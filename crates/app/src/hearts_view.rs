use cualsol_engine::{Card, Theme};
use cualsol_hearts::{seat_name, Game, HUMAN, PLAYERS};
use eframe::egui::{self, Pos2, Rect, Vec2};

use crate::paint::{draw_back, draw_face, Textures};

const DEAL_SPAN: f64 = 0.95;
const CARD_FLIGHT: f64 = 0.18;
const PLAY_FLIGHT: f64 = 0.28;
const GATHER_HOLD: f64 = 0.5;
const GATHER_FLIGHT: f64 = 0.34;

pub fn show(
    ui: &mut egui::Ui,
    game: &mut Game,
    theme: &Theme,
    textures: &mut Textures,
    now: f64,
    pace: &mut f64,
    dealt_hand: &mut u32,
    deal_start: &mut f64,
    trick_seen: &mut usize,
    play_at: &mut f64,
    gather_at: &mut f64,
    play_from: &mut Rect,
) {
    let rect = ui.max_rect();
    if *dealt_hand != game.hand_no {
        *dealt_hand = game.hand_no;
        *deal_start = now;
        *trick_seen = 0;
        *play_at = 0.0;
        *gather_at = 0.0;
    }
    let dealing = now - *deal_start < DEAL_SPAN + CARD_FLIGHT;
    if game.trick.is_empty() {
        *trick_seen = 0;
        *gather_at = 0.0;
    } else if game.trick.len() > *trick_seen {
        if *play_at == 0.0 {
            *play_at = now;
        }
        if now - *play_at >= PLAY_FLIGHT {
            *trick_seen = game.trick.len();
            *play_at = 0.0;
        }
    }
    let card_flying = game.trick.len() > *trick_seen;
    if dealing || card_flying || *gather_at > 0.0 {
        ui.ctx().request_repaint();
    }
    if game.trick_full() && !dealing && !card_flying && !game.finished {
        if *gather_at == 0.0 {
            *gather_at = now + GATHER_HOLD;
            ui.ctx().request_repaint();
        } else if now >= *gather_at + GATHER_FLIGHT {
            game.close_trick();
            *trick_seen = 0;
            *play_at = 0.0;
            *gather_at = 0.0;
            *pace = now + 0.35;
            ui.ctx().request_repaint();
        } else {
            ui.ctx().request_repaint();
        }
    } else if !dealing && !card_flying && *gather_at == 0.0 && !game.passing && !game.finished && game.turn != HUMAN && *pace <= now {
        let before = game.trick.len();
        game.play_ai();
        if game.trick.len() > before {
            *play_at = now;
        }
        *pace = now + 0.45;
        ui.ctx().request_repaint();
    } else if !dealing && !card_flying && *gather_at == 0.0 && !game.passing && !game.finished && game.turn != HUMAN {
        ui.ctx().request_repaint();
    }

    let painter = ui.painter_at(rect);
    painter.text(
        rect.left_top() + Vec2::new(16.0, 78.0),
        egui::Align2::LEFT_TOP,
        score_line(game),
        egui::FontId::proportional(18.0),
        egui::Color32::WHITE,
    );
    painter.text(
        rect.center_top() + Vec2::new(0.0, 108.0),
        egui::Align2::CENTER_TOP,
        &game.last_note,
        egui::FontId::proportional(16.0),
        egui::Color32::from_rgb(232, 196, 122),
    );

    let card = Vec2::new(78.0, 110.0);
    let center = rect.center() + Vec2::new(0.0, -20.0);
    if dealing {
        draw_deal(ui, game, theme, textures, rect, card, center, now, *deal_start);
        return;
    }
    let gathering = *gather_at > 0.0 && now >= *gather_at;
    let gather_t = if gathering {
        ((now - *gather_at) / GATHER_FLIGHT).clamp(0.0, 1.0) as f32
    } else {
        0.0
    };
    let taker = game.trick_taker().unwrap_or(0);
    for (index, (seat, played)) in game.trick.iter().enumerate() {
        let dest = trick_rect(*seat, center, card);
        let place = if gathering {
            let from = dest.min;
            let into = seat_pile(taker, rect, card).min;
            Rect::from_min_size(from + (into - from) * ease(gather_t), card)
        } else if index + 1 == game.trick.len() && index >= *trick_seen && *play_at > 0.0 {
            let t = ((now - *play_at) / PLAY_FLIGHT).clamp(0.0, 1.0) as f32;
            let from = if *seat == HUMAN && play_from.width() > 1.0 {
                *play_from
            } else {
                seat_pile(*seat, rect, card)
            };
            Rect::from_min_size(from.min + (dest.min - from.min) * ease(t), card)
        } else if index < *trick_seen {
            dest
        } else {
            continue;
        };
        draw_face(&painter, place, *played, theme, textures, ui.ctx(), true);
    }
    for seat in 1..PLAYERS {
        let count = game.hands[seat].len();
        let origin = match seat {
            1 => Pos2::new(rect.left() + 16.0, rect.center().y - 80.0),
            2 => Pos2::new(rect.center().x - 70.0, rect.top() + 140.0),
            _ => Pos2::new(rect.right() - 70.0, rect.center().y - 80.0),
        };
        if count > 0 {
            draw_back(&painter, Rect::from_min_size(origin, card), theme, textures, ui.ctx());
            painter.text(
                origin + Vec2::new(card.x / 2.0, card.y / 2.0),
                egui::Align2::CENTER_CENTER,
                format!("{count}"),
                egui::FontId::proportional(22.0),
                egui::Color32::WHITE,
            );
        }
    }

    let hand = game.hands[HUMAN].clone();
    let chosen = game.chosen.clone();
    let legal = if game.passing { hand.clone() } else { game.legal(HUMAN) };
    let width = card.x + (hand.len().saturating_sub(1) as f32) * 36.0;
    let mut x = rect.center().x - width / 2.0;
    let y = rect.bottom() - card.y - 18.0;
    for card_value in hand {
        let mut place = Rect::from_min_size(Pos2::new(x, y), card);
        if chosen.contains(&card_value) {
            place = place.translate(Vec2::new(0.0, -18.0));
        }
        let response = ui.interact(place, egui::Id::new(("corazon", card_id(card_value))), egui::Sense::click());
        draw_face(&painter, place, card_value, theme, textures, ui.ctx(), true);
        if legal.contains(&card_value) && response.hovered() {
            painter.rect_stroke(place, 6.0, egui::Stroke::new(2.0_f32, egui::Color32::from_rgb(232, 196, 122)), egui::StrokeKind::Inside);
        }
        if response.clicked() {
            if game.passing {
                game.toggle_pass(card_value);
            } else if game.turn == HUMAN && !card_flying && *gather_at == 0.0 {
                *play_from = place;
                if game.play_human(card_value) {
                    *play_at = now;
                    *pace = now + 0.3;
                }
            }
        }
        x += 36.0;
    }

    if game.passing && game.pass != cualsol_hearts::Pass::Hold {
        let button = Rect::from_center_size(Pos2::new(rect.center().x, y - 36.0), Vec2::new(120.0, 32.0));
        let response = ui.interact(button, egui::Id::new("pasar-corazones"), egui::Sense::click());
        painter.rect_filled(button, 8.0, egui::Color32::from_rgb(92, 64, 24));
        painter.text(
            button.center(),
            egui::Align2::CENTER_CENTER,
            "Pasar",
            egui::FontId::proportional(18.0),
            egui::Color32::WHITE,
        );
        if response.clicked() {
            game.confirm_pass();
        }
    }
}

fn draw_deal(
    ui: &mut egui::Ui,
    game: &Game,
    theme: &Theme,
    textures: &mut Textures,
    rect: Rect,
    card: Vec2,
    center: Pos2,
    now: f64,
    start: f64,
) {
    let painter = ui.painter_at(rect);
    let deck = Rect::from_center_size(center, card);
    draw_back(&painter, deck, theme, textures, ui.ctx());
    let hand = &game.hands[HUMAN];
    let width = card.x + 12.0 * 36.0;
    let hand_x = rect.center().x - width / 2.0;
    let hand_y = rect.bottom() - card.y - 18.0;
    for index in 0..52 {
        let depart = start + index as f64 * (DEAL_SPAN / 52.0);
        if now < depart {
            break;
        }
        let t = ((now - depart) / CARD_FLIGHT).clamp(0.0, 1.0) as f32;
        let eased = t * t * (3.0 - 2.0 * t);
        let seat = index % PLAYERS;
        let nth = index / PLAYERS;
        let dest = match seat {
            0 => Rect::from_min_size(Pos2::new(hand_x + nth as f32 * 36.0, hand_y), card),
            1 => Rect::from_min_size(Pos2::new(rect.left() + 16.0 + nth as f32 * 3.0, rect.center().y - 80.0), card),
            2 => Rect::from_min_size(Pos2::new(rect.center().x - 70.0 + nth as f32 * 3.0, rect.top() + 140.0), card),
            _ => Rect::from_min_size(Pos2::new(rect.right() - 70.0 - nth as f32 * 3.0, rect.center().y - 80.0), card),
        };
        let origin = deck.min + (dest.min - deck.min) * eased;
        let place = Rect::from_min_size(origin, card);
        if seat == HUMAN {
            if let Some(face) = hand.get(nth) {
                draw_face(&painter, place, *face, theme, textures, ui.ctx(), true);
            }
        } else {
            draw_back(&painter, place, theme, textures, ui.ctx());
        }
    }
}

fn trick_rect(seat: usize, center: Pos2, card: Vec2) -> Rect {
    let offset = match seat {
        0 => Vec2::new(0.0, 70.0),
        1 => Vec2::new(-90.0, 0.0),
        2 => Vec2::new(0.0, -70.0),
        _ => Vec2::new(90.0, 0.0),
    };
    Rect::from_center_size(center + offset, card)
}

fn seat_pile(seat: usize, rect: Rect, card: Vec2) -> Rect {
    let origin = match seat {
        0 => Pos2::new(rect.center().x - card.x / 2.0, rect.bottom() - card.y - 18.0),
        1 => Pos2::new(rect.left() + 16.0, rect.center().y - 80.0),
        2 => Pos2::new(rect.center().x - 70.0, rect.top() + 140.0),
        _ => Pos2::new(rect.right() - 70.0, rect.center().y - 80.0),
    };
    Rect::from_min_size(origin, card)
}

fn ease(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

fn score_line(game: &Game) -> String {
    (0..PLAYERS)
        .map(|seat| format!("{} {}", seat_name(seat), game.scores[seat]))
        .collect::<Vec<_>>()
        .join("    ")
}

fn card_id(card: Card) -> u16 {
    card.suit as u16 * 16 + card.rank.spider_value() as u16
}
