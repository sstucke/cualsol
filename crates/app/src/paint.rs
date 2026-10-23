use std::collections::HashMap;
use std::path::{Path, PathBuf};

use cualsol_engine::{BackPattern, Card, FaceStyle, Theme};

use crate::svgdeck::{card_element, SvgDeck, BACK_ELEMENT};
use cualsol_freecell::Game as FreeCell;
use cualsol_spider::{Game as Spider, COLUMNS};
use eframe::egui::{self, Color32, Painter, Pos2, Rect, TextureHandle, Vec2};

const FAN: f32 = 0.28;
const GAP: f32 = 8.0;

#[derive(Default)]
pub struct Textures {
    images: HashMap<PathBuf, TextureHandle>,
    pub face_svg: Option<SvgDeck>,
    pub back_svg: Option<SvgDeck>,
    svg_cache: HashMap<String, TextureHandle>,
}

impl Textures {
    pub fn release(&mut self) {
        self.images.clear();
        self.svg_cache.clear();
        self.face_svg = None;
        self.back_svg = None;
    }

    pub fn texture(&self, path: &Path) -> Option<&TextureHandle> {
        self.images.get(path)
    }

    pub fn ensure(&mut self, ctx: &egui::Context, path: &Path) {
        if path.is_file() && !self.images.contains_key(path) {
            if let Some(handle) = load_texture(ctx, path) {
                self.images.insert(path.to_path_buf(), handle);
            }
        }
    }

    pub fn reload(&mut self, ctx: &egui::Context, theme: &Theme) {
        self.images.clear();
        let mut paths = Vec::new();
        if let Some(path) = &theme.table.image {
            paths.push(path.clone());
        }
        if let Some(path) = &theme.back.image {
            paths.push(path.clone());
        }
        if let Some(dir) = &theme.faces.directory {
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    paths.push(entry.path());
                }
            }
        }
        for path in paths {
            if let Some(handle) = load_texture(ctx, &path) {
                self.images.insert(path, handle);
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct Board {
    pub card: Vec2,
    pub piles: Vec<Pile>,
    pub extras: Vec<Extra>,
}

#[derive(Clone, Debug)]
pub struct Pile {
    pub index: usize,
    pub slot: Rect,
    pub cards: Vec<Rect>,
}

#[derive(Clone, Copy, Debug)]
pub struct Extra {
    pub rect: Rect,
    pub kind: ExtraKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExtraKind {
    Stock,
    Waste,
    FreeCell(usize),
    Foundation(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeomHit {
    Card { column: usize, row: usize },
    EmptyColumn(usize),
    Extra(ExtraKind),
}

/// Cartas que se están arrastrando y no se pintan en su columna.
#[derive(Clone, Copy, Debug)]
pub struct Hidden {
    pub column: usize,
    pub from_row: usize,
}

pub fn spider_board(area: Rect, game: &Spider) -> Board {
    let tallest = game
        .tableau
        .iter()
        .map(|column| column.len())
        .max()
        .unwrap_or(1)
        .max(1) as f32;
    let card = fit_card(area, COLUMNS as f32, 2.15 + (tallest - 1.0) * FAN);
    let width = COLUMNS as f32 * card.x + (COLUMNS - 1) as f32 * GAP;
    let origin = Pos2::new(area.center().x - width / 2.0, area.top() + card.y + 12.0);
    let mut piles = Vec::with_capacity(COLUMNS);
    for (index, column) in game.tableau.iter().enumerate() {
        let x = origin.x + index as f32 * (card.x + GAP);
        let slot = Rect::from_min_size(Pos2::new(x, origin.y), card);
        let cards = (0..column.len())
            .map(|row| Rect::from_min_size(Pos2::new(x, origin.y + row as f32 * card.y * FAN), card))
            .collect();
        piles.push(Pile { index, slot, cards });
    }
    let stock = Rect::from_min_size(Pos2::new(area.left() + 8.0, area.top() + 8.0), card);
    Board {
        card,
        piles,
        extras: vec![Extra {
            rect: stock,
            kind: ExtraKind::Stock,
        }],
    }
}

pub fn freecell_board(area: Rect, game: &FreeCell) -> Board {
    let tallest = game
        .cascades
        .iter()
        .map(|column| column.len())
        .max()
        .unwrap_or(1)
        .max(1) as f32;
    let card = fit_card(area, 8.0, 2.2 + (tallest - 1.0) * FAN);
    let width = 8.0 * card.x + 7.0 * GAP;
    let left = area.center().x - width / 2.0;
    let top = area.top() + 8.0;
    let mut extras = Vec::with_capacity(8);
    for cell in 0..4 {
        extras.push(Extra {
            rect: Rect::from_min_size(Pos2::new(left + cell as f32 * (card.x + GAP), top), card),
            kind: ExtraKind::FreeCell(cell),
        });
    }
    for index in 0..4 {
        extras.push(Extra {
            rect: Rect::from_min_size(Pos2::new(left + (index + 4) as f32 * (card.x + GAP), top), card),
            kind: ExtraKind::Foundation(index),
        });
    }
    let cascade_top = top + card.y + 18.0;
    let mut piles = Vec::with_capacity(8);
    for (index, column) in game.cascades.iter().enumerate() {
        let x = left + index as f32 * (card.x + GAP);
        let slot = Rect::from_min_size(Pos2::new(x, cascade_top), card);
        let cards = (0..column.len())
            .map(|row| {
                Rect::from_min_size(Pos2::new(x, cascade_top + row as f32 * card.y * FAN), card)
            })
            .collect();
        piles.push(Pile { index, slot, cards });
    }
    Board { card, piles, extras }
}

pub const WASTE_PEEK: f32 = 28.0;

/// Posiciones del abanico del descarte: la última es la carta jugable.
pub fn waste_fan(slot: Rect, count: usize) -> Vec<Rect> {
    let shown = count.min(3);
    (0..shown)
        .map(|index| slot.translate(Vec2::new(index as f32 * WASTE_PEEK, 0.0)))
        .collect()
}

pub fn klondike_board(area: Rect, game: &cualsol_klondike::Game) -> Board {
    let tallest = game
        .tableau
        .iter()
        .map(|column| column.len())
        .max()
        .unwrap_or(1)
        .max(1) as f32;
    let card = fit_card(area, 7.0, 2.2 + (tallest - 1.0) * FAN);
    let width = 7.0 * card.x + 6.0 * GAP;
    let left = area.center().x - width / 2.0;
    let top = area.top() + 8.0;
    let mut extras = vec![
        Extra {
            rect: Rect::from_min_size(Pos2::new(left, top), card),
            kind: ExtraKind::Stock,
        },
        Extra {
            rect: waste_fan(Rect::from_min_size(Pos2::new(left + card.x + GAP, top), card), game.waste.len())
                .last()
                .copied()
                .unwrap_or_else(|| Rect::from_min_size(Pos2::new(left + card.x + GAP, top), card)),
            kind: ExtraKind::Waste,
        },
    ];
    for index in 0..4 {
        extras.push(Extra {
            rect: Rect::from_min_size(Pos2::new(left + (index + 3) as f32 * (card.x + GAP), top), card),
            kind: ExtraKind::Foundation(index),
        });
    }
    let cascade_top = top + card.y + 18.0;
    let mut piles = Vec::with_capacity(7);
    for (index, column) in game.tableau.iter().enumerate() {
        let x = left + index as f32 * (card.x + GAP);
        let slot = Rect::from_min_size(Pos2::new(x, cascade_top), card);
        let cards = (0..column.len())
            .map(|row| Rect::from_min_size(Pos2::new(x, cascade_top + row as f32 * card.y * FAN), card))
            .collect();
        piles.push(Pile { index, slot, cards });
    }
    Board { card, piles, extras }
}

pub fn draw_klondike(
    painter: &Painter,
    board: &Board,
    game: &cualsol_klondike::Game,
    theme: &Theme,
    textures: &mut Textures,
    ctx: &egui::Context,
    hidden: Option<Hidden>,
    conceal: Option<Hidden>,
    hide_foundation: Option<usize>,
    hide_waste: usize,
) {
    for extra in &board.extras {
        match extra.kind {
            ExtraKind::Stock => {
                if game.stock.is_empty() {
                    empty_slot(painter, extra.rect);
                } else {
                    draw_back(painter, extra.rect, theme, textures, ctx);
                }
            }
            ExtraKind::Waste => {
                let end = game.waste.len().saturating_sub(hide_waste);
                let start = end.saturating_sub(3);
                let visible = &game.waste[start..end];
                let full = game.waste.len().min(3).saturating_sub(1) as f32;
                let slot = Rect::from_min_size(
                    extra.rect.min - Vec2::new(full * WASTE_PEEK, 0.0),
                    extra.rect.size(),
                );
                if visible.is_empty() {
                    empty_slot(painter, slot);
                }
                for (index, card) in visible.iter().enumerate() {
                    draw_face(
                        painter,
                        slot.translate(Vec2::new(index as f32 * WASTE_PEEK, 0.0)),
                        *card,
                        theme,
                        textures,
                        ctx,
                        true,
                    );
                }
            }
            ExtraKind::Foundation(index) => match game.foundations[index] {
                Some(card) if hide_foundation != Some(index) => {
                    draw_face(painter, extra.rect, card, theme, textures, ctx, true)
                }
                _ => empty_slot(painter, extra.rect),
            },
            ExtraKind::FreeCell(_) => {}
        }
    }
    for pile in &board.piles {
        let column = &game.tableau[pile.index];
        if column.is_empty() {
            empty_slot(painter, pile.slot);
            continue;
        }
        for (row, (placed, rect)) in column.iter().zip(&pile.cards).enumerate() {
            if hidden.is_some_and(|hidden| hidden.column == pile.index && row >= hidden.from_row)
                || conceal.is_some_and(|hidden| hidden.column == pile.index && row >= hidden.from_row)
            {
                continue;
            }
            if placed.face_up {
                draw_face(painter, *rect, placed.card, theme, textures, ctx, true);
            } else {
                draw_back(painter, *rect, theme, textures, ctx);
            }
        }
    }
}

pub fn hit(board: &Board, pos: Pos2) -> Option<GeomHit> {
    for extra in &board.extras {
        if extra.rect.contains(pos) {
            return Some(GeomHit::Extra(extra.kind));
        }
    }
    for pile in board.piles.iter().rev() {
        for (row, rect) in pile.cards.iter().enumerate().rev() {
            let exposed = exposed_rect(*rect, row + 1 == pile.cards.len(), board.card.y * FAN);
            if exposed.contains(pos) {
                return Some(GeomHit::Card {
                    column: pile.index,
                    row,
                });
            }
        }
        if pile.cards.is_empty() && pile.slot.contains(pos) {
            return Some(GeomHit::EmptyColumn(pile.index));
        }
    }
    column_at(board, pos).map(GeomHit::EmptyColumn)
}

pub fn column_at(board: &Board, pos: Pos2) -> Option<usize> {
    for pile in board.piles.iter().rev() {
        if let Some(top) = pile.cards.last() {
            if top.contains(pos) {
                return Some(pile.index);
            }
        } else if pile.slot.contains(pos) {
            return Some(pile.index);
        }
    }
    board
        .piles
        .iter()
        .rev()
        .find(|pile| {
            let bottom = pile
                .cards
                .last()
                .map(|rect| rect.bottom())
                .unwrap_or(pile.slot.bottom());
            let mut zone = pile.slot;
            zone.set_bottom(bottom.max(pile.slot.bottom()) + 24.0);
            zone.min.x -= GAP / 2.0;
            zone.max.x += GAP / 2.0;
            zone.contains(pos)
        })
        .map(|pile| pile.index)
}

pub fn extra_at(board: &Board, pos: Pos2) -> Option<ExtraKind> {
    board
        .extras
        .iter()
        .find(|extra| extra.rect.contains(pos))
        .map(|extra| extra.kind)
}

fn exposed_rect(rect: Rect, is_top: bool, fan: f32) -> Rect {
    if is_top {
        rect
    } else {
        Rect::from_min_size(rect.min, Vec2::new(rect.width(), fan))
    }
}

pub fn draw_tablecloth(painter: &Painter, rect: Rect, theme: &Theme, textures: &Textures) {
    painter.rect_filled(rect, 0.0, rgb(theme.table.color));
    if let Some(path) = &theme.table.image {
        if let Some(texture) = textures.images.get(path) {
            painter.image(
                texture.id(),
                rect,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
        }
    }
}

pub fn draw_spider(
    painter: &Painter,
    area: Rect,
    board: &Board,
    game: &Spider,
    theme: &Theme,
    textures: &mut Textures,
    ctx: &egui::Context,
    hidden: Option<Hidden>,
    conceal: Option<Hidden>,
) {
    for pile in &board.piles {
        let column = &game.tableau[pile.index];
        if column.is_empty() {
            empty_slot(painter, pile.slot);
            continue;
        }
        for (row, (placed, rect)) in column.iter().zip(&pile.cards).enumerate() {
            if hidden.is_some_and(|hidden| hidden.column == pile.index && row >= hidden.from_row)
                || conceal.is_some_and(|hidden| hidden.column == pile.index && row >= hidden.from_row)
            {
                continue;
            }
            if placed.face_up {
                draw_face(painter, *rect, placed.card, theme, textures, ctx, true);
            } else {
                draw_back(painter, *rect, theme, textures, ctx);
            }
        }
    }
    for extra in &board.extras {
        if extra.kind != ExtraKind::Stock {
            continue;
        }
        if game.stock.is_empty() {
            empty_slot(painter, extra.rect);
        } else {
            draw_back(painter, extra.rect, theme, textures, ctx);
            painter.text(
                extra.rect.center(),
                egui::Align2::CENTER_CENTER,
                format!("{}", game.stock.len() / COLUMNS),
                egui::FontId::proportional(18.0),
                Color32::WHITE,
            );
        }
    }
    let mini = Vec2::new(board.card.x * 0.62, board.card.y * 0.62);
    let origin = Pos2::new(area.left() + 8.0, area.bottom() - mini.y - 8.0);
    for index in 0..game.completed.min(8) as usize {
        let rect = Rect::from_min_size(origin + Vec2::new(index as f32 * 16.0, 0.0), mini);
        draw_back(painter, rect, theme, textures, ctx);
    }
}

pub fn draw_freecell(
    painter: &Painter,
    board: &Board,
    game: &FreeCell,
    theme: &Theme,
    textures: &mut Textures,
    ctx: &egui::Context,
    hidden_column: Option<Hidden>,
    hidden_cell: Option<usize>,
    conceal: Option<Hidden>,
    hide_foundation: Option<usize>,
) {
    for extra in &board.extras {
        match extra.kind {
            ExtraKind::FreeCell(index) => match game.freecells[index] {
                Some(card) if hidden_cell != Some(index) => {
                    draw_face(painter, extra.rect, card, theme, textures, ctx, true)
                }
                _ => empty_slot(painter, extra.rect),
            },
            ExtraKind::Foundation(index) => match game.foundations[index] {
                Some(card) if hide_foundation != Some(index) => {
                    draw_face(painter, extra.rect, card, theme, textures, ctx, true)
                }
                _ => empty_slot(painter, extra.rect),
            },
            ExtraKind::Stock | ExtraKind::Waste => {}
        }
    }
    for pile in &board.piles {
        let column = &game.cascades[pile.index];
        if column.is_empty() {
            empty_slot(painter, pile.slot);
            continue;
        }
        for (row, (placed, rect)) in column.iter().zip(&pile.cards).enumerate() {
            if hidden_column
                .is_some_and(|hidden| hidden.column == pile.index && row >= hidden.from_row)
                || conceal.is_some_and(|hidden| hidden.column == pile.index && row >= hidden.from_row)
            {
                continue;
            }
            draw_face(painter, *rect, placed.card, theme, textures, ctx, true);
        }
    }
}

pub fn draw_face(
    painter: &Painter,
    rect: Rect,
    card: Card,
    theme: &Theme,
    textures: &mut Textures,
    ctx: &egui::Context,
    svg: bool,
) {
    if svg {
        if let Some(texture) = svg_card(textures, ctx, true, &card_element(card), rect.width()) {
            paint_image_card(painter, rect, &texture);
            return;
        }
    }
    if let Some(path) = theme.faces.image_for(card) {
        if let Some(texture) = textures.images.get(&path) {
            paint_image_card(painter, rect, texture);
            return;
        }
    }
    let (paper, ink) = face_colors(theme.faces.style, card);
    painter.rect_filled(rect, 7.0, paper);
    painter.rect_stroke(
        rect,
        7.0,
        egui::Stroke::new(1.0_f32, Color32::from_black_alpha(90)),
        egui::StrokeKind::Inside,
    );
    painter.rect_stroke(
        rect.shrink(4.0),
        5.0,
        egui::Stroke::new(1.0_f32, ink.gamma_multiply(0.45)),
        egui::StrokeKind::Inside,
    );
    let corner = format!("{}\n{}", card.rank.label(), card.suit.symbol());
    painter.text(
        rect.left_top() + Vec2::new(7.0, 5.0),
        egui::Align2::LEFT_TOP,
        &corner,
        egui::FontId::proportional(font_px(rect.height() * 0.16)),
        ink,
    );
    match theme.faces.style {
        FaceStyle::Grande => {
            painter.text(
                rect.center() + Vec2::new(0.0, -rect.height() * 0.06),
                egui::Align2::CENTER_CENTER,
                card.rank.label(),
                egui::FontId::proportional(font_px(rect.height() * 0.46)),
                ink,
            );
            painter.text(
                rect.center() + Vec2::new(0.0, rect.height() * 0.24),
                egui::Align2::CENTER_CENTER,
                card.suit.symbol(),
                egui::FontId::proportional(font_px(rect.height() * 0.2)),
                ink,
            );
        }
        _ => {
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                card.suit.symbol(),
                egui::FontId::proportional(font_px(rect.height() * 0.42)),
                ink,
            );
        }
    }
}

fn face_colors(style: FaceStyle, card: Card) -> (Color32, Color32) {
    let red = Color32::from_rgb(176, 32, 40);
    let black = Color32::from_rgb(24, 24, 28);
    match style {
        FaceStyle::Night => (
            Color32::from_rgb(28, 34, 46),
            if card.suit.is_red() {
                Color32::from_rgb(232, 170, 138)
            } else {
                Color32::from_rgb(236, 226, 204)
            },
        ),
        FaceStyle::FourColor => {
            let ink = match card.suit {
                cualsol_engine::Suit::Clubs => Color32::from_rgb(18, 122, 72),
                cualsol_engine::Suit::Diamonds => Color32::from_rgb(32, 86, 176),
                cualsol_engine::Suit::Hearts => red,
                cualsol_engine::Suit::Spades => black,
            };
            (Color32::from_rgb(250, 248, 242), ink)
        }
        FaceStyle::Classic | FaceStyle::Grande => (
            Color32::from_rgb(248, 246, 240),
            if card.suit.is_red() { red } else { black },
        ),
    }
}

pub fn draw_back(
    painter: &Painter,
    rect: Rect,
    theme: &Theme,
    textures: &mut Textures,
    ctx: &egui::Context,
) {
    if let Some(texture) = svg_card(textures, ctx, false, BACK_ELEMENT, rect.width()) {
        paint_image_card(painter, rect, &texture);
        return;
    }
    if let Some(path) = &theme.back.image {
        if let Some(texture) = textures.images.get(path) {
            paint_image_card(painter, rect, texture);
            return;
        }
    }
    let color = rgb(theme.back.color);
    painter.rect_filled(rect, 6.0, color);
    painter.rect_stroke(
        rect.shrink(4.0),
        4.0,
        egui::Stroke::new(1.5_f32, Color32::from_white_alpha(180)),
        egui::StrokeKind::Inside,
    );
    match theme.back.pattern {
        BackPattern::Solid => {}
        BackPattern::Diamond => {
            let mid = rect.center();
            let diamond = [
                mid + Vec2::new(0.0, -rect.height() * 0.18),
                mid + Vec2::new(rect.width() * 0.16, 0.0),
                mid + Vec2::new(0.0, rect.height() * 0.18),
                mid + Vec2::new(-rect.width() * 0.16, 0.0),
            ];
            painter.add(egui::Shape::convex_polygon(
                diamond.to_vec(),
                Color32::from_white_alpha(40),
                egui::Stroke::new(1.0_f32, Color32::from_white_alpha(160)),
            ));
        }
        BackPattern::Lines => {
            let mut y = rect.top() + 10.0;
            while y < rect.bottom() - 6.0 {
                painter.line_segment(
                    [Pos2::new(rect.left() + 8.0, y), Pos2::new(rect.right() - 8.0, y)],
                    egui::Stroke::new(1.0_f32, Color32::from_white_alpha(90)),
                );
                y += 7.0;
            }
        }
    }
}

pub fn highlight(painter: &Painter, rect: Rect) {
    painter.rect_stroke(
        rect,
        6.0,
        egui::Stroke::new(2.5_f32, Color32::from_rgb(255, 214, 90)),
        egui::StrokeKind::Inside,
    );
}

fn svg_card(
    textures: &mut Textures,
    ctx: &egui::Context,
    face: bool,
    id: &str,
    width_points: f32,
) -> Option<TextureHandle> {
    let pixels_wide = (width_points * ctx.pixels_per_point()).round().clamp(8.0, 400.0) as u32;
    let key = format!("{}:{id}@{pixels_wide}", if face { "f" } else { "b" });
    if let Some(texture) = textures.svg_cache.get(&key) {
        return Some(texture.clone());
    }
    let rendered = {
        let deck = if face {
            textures.face_svg.as_ref()
        } else {
            textures.back_svg.as_ref()
        }?;
        if !deck.has(id) {
            return None;
        }
        deck.render(id, pixels_wide)?
    };
    let height = rendered.len() / (pixels_wide as usize * 4);
    if height == 0 {
        return None;
    }
    let image = egui::ColorImage::from_rgba_premultiplied([pixels_wide as usize, height], &rendered);
    let handle = ctx.load_texture(&key, image, egui::TextureOptions::LINEAR);
    textures.svg_cache.insert(key, handle.clone());
    Some(handle)
}

fn paint_image_card(painter: &Painter, rect: Rect, texture: &TextureHandle) {
    painter.image(
        texture.id(),
        rect,
        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
        Color32::WHITE,
    );
}

fn empty_slot(painter: &Painter, rect: Rect) {
    painter.rect_stroke(
        rect,
        6.0,
        egui::Stroke::new(1.5_f32, Color32::from_white_alpha(70)),
        egui::StrokeKind::Inside,
    );
}

fn fit_card(area: Rect, columns: f32, rows_budget: f32) -> Vec2 {
    let max_w = (area.width() - GAP * (columns - 1.0) - 16.0) / columns;
    let max_h = (area.height() - 16.0) / rows_budget.max(1.0);
    let height = max_h.min(max_w * 1.4).max(32.0).round();
    Vec2::new((height / 1.4).round(), height)
}

fn font_px(size: f32) -> f32 {
    (size / 2.0).round().clamp(4.0, 48.0) * 2.0
}

fn rgb(color: cualsol_engine::Rgb) -> Color32 {
    Color32::from_rgb(color.r, color.g, color.b)
}

fn load_texture(ctx: &egui::Context, path: &Path) -> Option<TextureHandle> {
    let bytes = std::fs::read(path).ok()?;
    let image = image::load_from_memory(&bytes).ok()?.to_rgba8();
    let size = [image.width() as usize, image.height() as usize];
    let color = egui::ColorImage::from_rgba_unmultiplied(size, image.as_raw());
    Some(ctx.load_texture(
        path.display().to_string(),
        color,
        egui::TextureOptions::LINEAR,
    ))
}
