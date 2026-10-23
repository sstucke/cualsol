#![windows_subsystem = "windows"]

mod hearts_view;
mod icons;
mod paint;
mod svgdeck;

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use cualsol_engine::{load_theme, Card, FaceStyle, Rank, Suit, Theme};
use cualsol_freecell::{Game as FreeCell, Move as FreeMove, Source};
use cualsol_hearts::Game as Hearts;
use cualsol_klondike::{Game as Klondike, Move as KMove};
use cualsol_spider::{Game as Spider, Move as SpiderMove, Variant};
use eframe::egui::{self, Key, Modifiers, Pos2, Rect, Vec2};
use serde::{Deserialize, Serialize};

use crate::icons::{token, Icon};
use crate::paint::{
    draw_back, draw_face, draw_freecell, draw_klondike, draw_spider, draw_tablecloth, extra_at,
    freecell_board, highlight, hit, klondike_board, spider_board, waste_fan, Board, ExtraKind, GeomHit,
    Hidden, Textures, WASTE_PEEK,
};


const FAN: f32 = 0.28;

enum Table {
    Spider(Spider),
    FreeCell(FreeCell),
    Klondike(Klondike),
    Hearts(Hearts),
}

struct History {
    undo: Vec<Table>,
    redo: Vec<Table>,
}

impl History {
    fn new() -> Self {
        Self {
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }

    fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }
}

enum DragSource {
    Spider { column: usize, count: usize },
    Cascade { column: usize, count: usize },
    Klondike { column: usize, count: usize },
    Waste,
    FreeCell(usize),
}

struct Drag {
    cards: Vec<Card>,
    size: Vec2,
    offset: Vec2,
    pointer: Pos2,
    source: DragSource,
}

struct Press {
    pos: Pos2,
    hit: GeomHit,
}

struct Piece {
    card: Card,
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    trail: Vec<(f32, f32)>,
    rest: bool,
}

struct WinAnim {
    last: f64,
    next_spawn: f64,
    spawned: usize,
    pieces: Vec<Piece>,
}

struct Flight {
    cards: Vec<Card>,
    from: Vec<Rect>,
    to: Vec<Rect>,
    start: f64,
    secs: f64,
    stagger: f64,
    conceal: Option<Hidden>,
    foundation: Option<usize>,
    waste_hide: usize,
}

const FLIGHT_SECS: f64 = 0.22;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GameKind {
    Spider1,
    Spider2,
    Spider4,
    FreeCell,
    Klondike,
    Hearts,
}

impl GameKind {
    fn id(self) -> &'static str {
        match self {
            GameKind::Spider1 => "spider1",
            GameKind::Spider2 => "spider2",
            GameKind::Spider4 => "spider4",
            GameKind::FreeCell => "freecell",
            GameKind::Klondike => "klondike",
            GameKind::Hearts => "corazones",
        }
    }

    fn name(self) -> &'static str {
        match self {
            GameKind::Spider1 => "Spider de 1 palo",
            GameKind::Spider2 => "Spider de 2 palos",
            GameKind::Spider4 => "Spider de 4 palos",
            GameKind::FreeCell => "FreeCell",
            GameKind::Klondike => "Klondike",
            GameKind::Hearts => "Corazones",
        }
    }

    fn parse(value: &str) -> Self {
        match value {
            "spider1" => GameKind::Spider1,
            "spider2" => GameKind::Spider2,
            "freecell" => GameKind::FreeCell,
            "klondike" => GameKind::Klondike,
            "corazones" => GameKind::Hearts,
            _ => GameKind::Spider4,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct Opciones {
    #[serde(default)]
    game: Option<String>,
    #[serde(default)]
    face: Option<String>,
    #[serde(default)]
    back: Option<String>,
    #[serde(default)]
    felt: Option<String>,
    #[serde(default)]
    auto: bool,
}

struct CualApp {
    theme: Theme,
    textures: Textures,
    table: Table,
    kind: GameKind,
    history: History,
    drag: Option<Drag>,
    press: Option<Press>,
    show_options: bool,
    show_games: bool,
    auto: bool,
    face_id: String,
    back_id: String,
    felt_id: String,
    status: String,
    play_area: Rect,
    flight: Option<Flight>,
    win: Option<WinAnim>,
    pace: f64,
    hearts_hand: u32,
    hearts_deal: f64,
    hearts_seen: usize,
    hearts_play: f64,
    hearts_gather: f64,
    hearts_from: Rect,
}

impl CualApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut style = (*cc.egui_ctx.style()).clone();
        style.spacing.button_padding = egui::vec2(10.0, 6.0);
        cc.egui_ctx.set_style(style);
        let mut theme = open_default_theme();
        let opciones = load_opciones();
        let kind = opciones
            .game
            .as_deref()
            .map(GameKind::parse)
            .unwrap_or(GameKind::Spider4);
        let face_id = opciones.face.unwrap_or_else(|| "clasico".to_string());
        let back_id = opciones.back.unwrap_or_else(|| "dorso1".to_string());
        let felt_id = opciones.felt.unwrap_or_else(|| "tapete1".to_string());
        apply_look(&mut theme, &face_id, &back_id, &felt_id);
        let mut app = Self {
            theme,
            textures: Textures::default(),
            table: Table::Spider(Spider::deal(Variant::FourSuits, 1)),
            kind,
            history: History::new(),
            drag: None,
            press: None,
            show_options: false,
            show_games: false,
            auto: opciones.auto,
            face_id,
            back_id,
            felt_id,
            status: String::new(),
            play_area: Rect::from_min_size(Pos2::ZERO, egui::vec2(800.0, 600.0)),
            flight: None,
            win: None,
            pace: 0.0,
            hearts_hand: u32::MAX,
            hearts_deal: 0.0,
            hearts_seen: 0,
            hearts_play: 0.0,
            hearts_gather: 0.0,
            hearts_from: Rect::ZERO,
        };
        if let Some(saved) = load_partida() {
            app.kind = match &saved {
                Table::Spider(game) => match game.variant {
                    Variant::OneSuit => GameKind::Spider1,
                    Variant::TwoSuits => GameKind::Spider2,
                    Variant::FourSuits => GameKind::Spider4,
                },
                Table::FreeCell(_) => GameKind::FreeCell,
                Table::Klondike(_) => GameKind::Klondike,
                Table::Hearts(_) => GameKind::Hearts,
            };
            app.table = saved;
        } else {
            let table = app.table_for(kind, fresh_seed());
            app.deal(table);
        }
        app.textures.reload(&cc.egui_ctx, &app.theme);
        app.warm(&cc.egui_ctx);
        app
    }

    fn ensure_kde_previews(&mut self, ctx: &egui::Context) {
        for deck in KDE {
            self.textures.ensure(ctx, &asset(deck.preview));
            self.textures.ensure(ctx, &asset(&format!("{}/back.png", deck.raster)));
        }
    }

    fn table_for(&self, kind: GameKind, seed: u64) -> Table {
        match kind {
            GameKind::Spider1 => Table::Spider(Spider::deal(Variant::OneSuit, seed)),
            GameKind::Spider2 => Table::Spider(Spider::deal(Variant::TwoSuits, seed)),
            GameKind::Spider4 => Table::Spider(Spider::deal(Variant::FourSuits, seed)),
            GameKind::FreeCell => Table::FreeCell(FreeCell::deal(seed)),
            GameKind::Klondike => Table::Klondike(Klondike::deal(seed)),
            GameKind::Hearts => Table::Hearts(Hearts::deal(seed)),
        }
    }

    fn choose_game(&mut self, kind: GameKind) {
        self.kind = kind;
        self.deal(self.table_for(kind, fresh_seed()));
        self.show_games = false;
        save_opciones(self);
    }

    fn restart(&mut self) {
        let seed = match &self.table {
            Table::Spider(game) => game.seed,
            Table::FreeCell(game) => game.seed,
            Table::Klondike(game) => game.seed,
            Table::Hearts(game) => game.seed,
        };
        let kind = self.kind;
        self.deal(self.table_for(kind, seed));
    }

    fn reshuffle(&mut self) {
        let kind = self.kind;
        self.deal(self.table_for(kind, fresh_seed()));
    }

    fn deal(&mut self, table: Table) {
        self.table = table;
        self.history.clear();
        self.drag = None;
        self.press = None;
        self.flight = None;
        self.win = None;
        self.pace = 0.0;
        self.hearts_hand = u32::MAX;
        self.hearts_seen = 0;
        self.hearts_play = 0.0;
        self.hearts_gather = 0.0;
        self.hearts_from = Rect::ZERO;
        self.status.clear();
    }

    fn undo(&mut self) {
        let Some(previous) = self.history.undo.pop() else {
            return;
        };
        self.history.redo.push(snapshot(&self.table));
        self.table = previous;
        self.drag = None;
        self.flight = None;
        self.win = None;
        self.status.clear();
    }

    fn redo(&mut self) {
        let Some(next) = self.history.redo.pop() else {
            return;
        };
        self.history.undo.push(snapshot(&self.table));
        self.table = next;
        self.drag = None;
        self.flight = None;
        self.win = None;
        self.status.clear();
    }

    fn deal_stock(&mut self, ctx: &egui::Context) {
        let before = snapshot(&self.table);
        let drawn = self.animate_klondike_draw(ctx);
        let ok = drawn
            || match &mut self.table {
                Table::Spider(game) => game.apply(SpiderMove::DealStock).is_ok(),
                Table::Klondike(game) => game.stock.is_empty() && game.apply(KMove::Recycle).is_ok(),
                Table::FreeCell(_) | Table::Hearts(_) => false,
            };
        if ok {
            self.finish_move(before, ctx);
        } else {
            self.status = "Hace falta una carta en cada columna para robar.".to_string();
        }
    }

    fn animate_klondike_draw(&mut self, ctx: &egui::Context) -> bool {
        let Table::Klondike(game) = &self.table else {
            return false;
        };
        if game.stock.is_empty() {
            return false;
        }
        let started = ctx.input(|input| input.time);
        let before = klondike_board(self.play_area, game);
        let stock = extra_rect(&before, ExtraKind::Stock).unwrap_or(before.piles[0].slot);
        let count = game.stock.len().min(3);
        let cards: Vec<Card> = game.stock[game.stock.len() - count..].iter().rev().copied().collect();
        let Table::Klondike(game) = &mut self.table else {
            return false;
        };
        if game.apply(KMove::Draw).is_err() {
            return false;
        }
        let after = klondike_board(self.play_area, game);
        let fan = waste_fan(
            Rect::from_min_size(
                after
                    .extras
                    .iter()
                    .find(|extra| extra.kind == ExtraKind::Waste)
                    .map(|extra| extra.rect.min)
                    .unwrap_or(stock.min)
                    - Vec2::new((game.waste.len().min(3).saturating_sub(1) as f32) * WASTE_PEEK, 0.0),
                stock.size(),
            ),
            game.waste.len().min(3),
        );
        let to = fan[fan.len().saturating_sub(count)..].to_vec();
        let stagger = 0.08;
        self.flight = Some(Flight {
            cards,
            from: vec![stock; count],
            to,
            start: started,
            secs: FLIGHT_SECS + stagger * (count.saturating_sub(1) as f64),
            stagger,
            conceal: None,
            foundation: None,
            waste_hide: count,
        });
        ctx.request_repaint();
        true
    }

    fn finish_move(&mut self, before: Table, ctx: &egui::Context) {
        self.history.undo.push(before);
        self.history.redo.clear();
        self.status.clear();
        self.launch_auto(ctx);
    }

    fn launch_auto(&mut self, ctx: &egui::Context) {
        if !self.auto || self.flight.is_some() {
            return;
        }
        match &self.table {
            Table::Spider(game) => {
                let Some(SpiderMove::Transfer { from, count, to }) = game.next_same_suit_build() else {
                    return;
                };
                self.glide_spider(ctx, from, count, to);
            }
            Table::FreeCell(game) => {
                let Some(movement) = game.next_safe_foundation() else {
                    return;
                };
                self.glide_free(ctx, movement);
            }
            Table::Klondike(game) => {
                let Some(movement) = game.next_foundation() else {
                    return;
                };
                self.glide_klondike(ctx, movement);
            }
            Table::Hearts(_) => {}
        }
    }

    fn glide_klondike(&mut self, ctx: &egui::Context, movement: KMove) -> bool {
        let Table::Klondike(game) = &mut self.table else {
            return false;
        };
        let started = ctx.input(|input| input.time);
        let before = klondike_board(self.play_area, game);
        let fallback = Rect::from_min_size(Pos2::ZERO, before.card);
        let (cards, from_rects, foundation, conceal_to) = match movement {
            KMove::WasteToFoundation => {
                let Some(card) = game.waste.last().copied() else {
                    return false;
                };
                let rect = extra_rect(&before, ExtraKind::Waste).unwrap_or(fallback);
                (vec![card], vec![rect], Some(foundation_index(card)), None)
            }
            KMove::TableauToFoundation { from } => {
                let Some(placed) = game.tableau[from].last().copied() else {
                    return false;
                };
                let rect = before.piles[from].cards.last().copied().unwrap_or(fallback);
                (vec![placed.card], vec![rect], Some(foundation_index(placed.card)), None)
            }
            KMove::WasteToTableau { to } => {
                let Some(card) = game.waste.last().copied() else {
                    return false;
                };
                let rect = extra_rect(&before, ExtraKind::Waste).unwrap_or(fallback);
                (vec![card], vec![rect], None, Some(to))
            }
            KMove::Tableau { from, count, to } => {
                let row = game.tableau[from].len().saturating_sub(count);
                let cards: Vec<Card> = game.tableau[from][row..].iter().map(|placed| placed.card).collect();
                let rects = before.piles[from].cards[row..].to_vec();
                (cards, rects, None, Some(to))
            }
            KMove::Draw | KMove::Recycle => return false,
        };
        if game.apply(movement).is_err() {
            return false;
        }
        let after = klondike_board(self.play_area, game);
        let (to_rects, conceal) = if let Some(index) = foundation {
            let rect = extra_rect(&after, ExtraKind::Foundation(index)).unwrap_or(fallback);
            (vec![rect], None)
        } else if let Some(to) = conceal_to {
            let count = cards.len();
            let start = after.piles[to].cards.len().saturating_sub(count);
            (after.piles[to].cards[start..].to_vec(), Some(Hidden { column: to, from_row: start }))
        } else {
            (from_rects.clone(), None)
        };
        self.flight = Some(Flight {
            cards,
            from: from_rects,
            to: to_rects,
            start: started,
            secs: FLIGHT_SECS,
            stagger: 0.0,
            conceal,
            foundation,
            waste_hide: 0,
        });
        ctx.request_repaint();
        true
    }

    fn glide_spider(&mut self, ctx: &egui::Context, from: usize, count: usize, to: usize) -> bool {
        let Table::Spider(game) = &mut self.table else {
            return false;
        };
        if count == 0 || game.tableau[from].len() < count {
            return false;
        }
        let started = ctx.input(|input| input.time);
        let before = spider_board(self.play_area, game);
        let row = game.tableau[from].len() - count;
        let cards: Vec<Card> = game.tableau[from][row..].iter().map(|placed| placed.card).collect();
        let from_rects: Vec<Rect> = before.piles[from].cards[row..].to_vec();
        let completed = game.completed;
        if game.apply(SpiderMove::Transfer { from, count, to }).is_err() {
            return false;
        }
        let after = spider_board(self.play_area, game);
        let to_rects = if game.completed > completed {
            completed_rects(self.play_area, after.card, cards.len())
        } else {
            let start = after.piles[to].cards.len().saturating_sub(count);
            after.piles[to].cards[start..].to_vec()
        };
        self.flight = Some(Flight {
            cards,
            from: from_rects,
            to: to_rects,
            start: started,
            secs: FLIGHT_SECS,
            stagger: 0.0,
            conceal: (game.completed == completed).then_some(Hidden {
                column: to,
                from_row: after.piles[to].cards.len().saturating_sub(count),
            }),
            foundation: None,
            waste_hide: 0,
        });
        ctx.request_repaint();
        true
    }

    fn glide_free(&mut self, ctx: &egui::Context, movement: FreeMove) -> bool {
        let Table::FreeCell(game) = &mut self.table else {
            return false;
        };
        let started = ctx.input(|input| input.time);
        let before = freecell_board(self.play_area, game);
        let fallback = Rect::from_min_size(Pos2::ZERO, before.card);
        let (cards, from_rects) = match movement {
            FreeMove::Run { from: column, count, .. } => {
                let row = game.cascades[column].len().saturating_sub(count);
                let cards: Vec<Card> = game.cascades[column][row..].iter().map(|placed| placed.card).collect();
                let rects = before.piles[column].cards[row..].to_vec();
                (cards, rects)
            }
            FreeMove::ToFoundation { from: Source::Cascade(column) }
            | FreeMove::CascadeToFreecell { from: column, .. } => {
                let row = game.cascades[column].len().saturating_sub(1);
                let cards: Vec<Card> = game.cascades[column][row..].iter().map(|placed| placed.card).collect();
                let rects = before.piles[column].cards[row..].to_vec();
                (cards, rects)
            }
            FreeMove::ToFoundation { from: Source::Freecell(cell) }
            | FreeMove::FreecellToCascade { cell, .. }
            | FreeMove::FreecellToFreecell { from: cell, .. } => {
                let Some(card) = game.freecells[cell] else {
                    return false;
                };
                let rect = before
                    .extras
                    .iter()
                    .find(|extra| extra.kind == ExtraKind::FreeCell(cell))
                    .map(|extra| extra.rect)
                    .unwrap_or(fallback);
                (vec![card], vec![rect])
            }
            FreeMove::CascadeToCascade { .. } => return false,
        };
        if game.apply(movement).is_err() {
            return false;
        }
        let after = freecell_board(self.play_area, game);
        let (to_rects, conceal, foundation) = match movement {
            FreeMove::ToFoundation { from } => {
                let card = *cards.last().unwrap_or(&Card::new(Suit::Spades, Rank::Ace));
                let index = foundation_index(card);
                let rect = after
                    .extras
                    .iter()
                    .find(|extra| extra.kind == ExtraKind::Foundation(index))
                    .map(|extra| extra.rect)
                    .unwrap_or(fallback);
                (vec![rect], None, Some(index))
            }
            FreeMove::Run { to, count, .. } => {
                let start = after.piles[to].cards.len().saturating_sub(count);
                (after.piles[to].cards[start..].to_vec(), Some(Hidden { column: to, from_row: start }), None)
            }
            FreeMove::CascadeToFreecell { cell, .. } | FreeMove::FreecellToFreecell { to: cell, .. } => {
                let rect = after
                    .extras
                    .iter()
                    .find(|extra| extra.kind == ExtraKind::FreeCell(cell))
                    .map(|extra| extra.rect)
                    .unwrap_or(fallback);
                (vec![rect], None, None)
            }
            FreeMove::FreecellToCascade { to, .. } => {
                let start = after.piles[to].cards.len().saturating_sub(1);
                (after.piles[to].cards[start..].to_vec(), Some(Hidden { column: to, from_row: start }), None)
            }
            FreeMove::CascadeToCascade { .. } => return false,
        };
        self.flight = Some(Flight {
            cards,
            from: from_rects,
            to: to_rects,
            start: started,
            secs: FLIGHT_SECS,
            stagger: 0.0,
            conceal,
            foundation,
            waste_hide: 0,
        });
        ctx.request_repaint();
        true
    }

    fn begin_drag(&mut self, board: &Board, pos: Pos2) {
        let Some(hit) = hit(board, pos) else {
            return;
        };
        let drag = match (&self.table, hit) {
            (Table::Spider(game), GeomHit::Card { column, row }) => {
                let Some(count) = game.liftable_from(column, row) else {
                    return;
                };
                let Some(rect) = board
                    .piles
                    .iter()
                    .find(|pile| pile.index == column)
                    .and_then(|pile| pile.cards.get(row))
                else {
                    return;
                };
                let cards = game.tableau[column][row..]
                    .iter()
                    .map(|placed| placed.card)
                    .collect();
                Drag {
                    cards,
                    size: board.card,
                    offset: pos - rect.min,
                    pointer: pos,
                    source: DragSource::Spider { column, count },
                }
            }
            (Table::FreeCell(game), GeomHit::Card { column, row }) => {
                let Some(count) = game.liftable_from(column, row) else {
                    return;
                };
                let Some(rect) = board
                    .piles
                    .iter()
                    .find(|pile| pile.index == column)
                    .and_then(|pile| pile.cards.get(row))
                else {
                    return;
                };
                let cards = game.cascades[column][row..]
                    .iter()
                    .map(|placed| placed.card)
                    .collect();
                Drag {
                    cards,
                    size: board.card,
                    offset: pos - rect.min,
                    pointer: pos,
                    source: DragSource::Cascade { column, count },
                }
            }
            (Table::Klondike(game), GeomHit::Card { column, row }) => {
                let Some(count) = game.liftable_from(column, row) else {
                    return;
                };
                let Some(rect) = board.piles.iter().find(|pile| pile.index == column).and_then(|pile| pile.cards.get(row)) else {
                    return;
                };
                let cards = game.tableau[column][row..].iter().map(|placed| placed.card).collect();
                Drag {
                    cards,
                    size: board.card,
                    offset: pos - rect.min,
                    pointer: pos,
                    source: DragSource::Klondike { column, count },
                }
            }
            (Table::Klondike(game), GeomHit::Extra(ExtraKind::Waste)) => {
                let Some(card) = game.waste.last().copied() else {
                    return;
                };
                let Some(rect) = extra_rect(board, ExtraKind::Waste) else {
                    return;
                };
                Drag {
                    cards: vec![card],
                    size: board.card,
                    offset: pos - rect.min,
                    pointer: pos,
                    source: DragSource::Waste,
                }
            }
            (Table::FreeCell(game), GeomHit::Extra(ExtraKind::FreeCell(cell))) => {
                let Some(card) = game.freecells[cell] else {
                    return;
                };
                let Some(rect) = board
                    .extras
                    .iter()
                    .find(|extra| extra.kind == ExtraKind::FreeCell(cell))
                    .map(|extra| extra.rect)
                else {
                    return;
                };
                Drag {
                    cards: vec![card],
                    size: board.card,
                    offset: pos - rect.min,
                    pointer: pos,
                    source: DragSource::FreeCell(cell),
                }
            }
            _ => return,
        };
        self.drag = Some(drag);
    }

    fn drop_at(&mut self, board: &Board, pos: Pos2, ctx: &egui::Context) {
        let Some(drag) = self.drag.take() else {
            return;
        };
        let before = snapshot(&self.table);
        let moved = match (&mut self.table, &drag.source) {
            (Table::Spider(game), DragSource::Spider { column, count }) => {
                match column_under(board, pos) {
                    Some(to) if to != *column => game
                        .apply(SpiderMove::Transfer {
                            from: *column,
                            count: *count,
                            to,
                        })
                        .is_ok(),
                    _ => false,
                }
            }
            (Table::FreeCell(game), DragSource::Cascade { column, count }) => {
                drop_freecell_from_cascade(game, board, pos, *column, *count)
            }
            (Table::FreeCell(game), DragSource::FreeCell(cell)) => {
                drop_freecell_from_cell(game, board, pos, *cell)
            }
            (Table::Klondike(game), DragSource::Klondike { column, count }) => {
                drop_klondike_run(game, board, pos, *column, *count)
            }
            (Table::Klondike(game), DragSource::Waste) => drop_klondike_waste(game, board, pos),
            _ => false,
        };
        if moved {
            self.finish_move(before, ctx);
        }
    }

    fn warm(&mut self, ctx: &egui::Context) {
        for id in BACKS {
            self.textures.ensure(ctx, &asset(&format!("dorsos/{id}.jpg")));
        }
        for id in FELTS {
            self.textures.ensure(ctx, &asset(&format!("tapetes/{id}.jpg")));
        }
    }

    fn options_panel(&mut self, ctx: &egui::Context) {
        if !self.show_options {
            return;
        }
        self.warm(ctx);
        self.ensure_kde_previews(ctx);
        let mut changed = false;
        let mut close = false;
        egui::SidePanel::left(egui::Id::new("ventana-opciones"))
            .resizable(true)
            .default_width(332.0)
            .width_range(280.0..=460.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Opciones").strong());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("×").on_hover_text("Cerrar").clicked() {
                            close = true;
                        }
                    });
                });
                ui.separator();
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                ui.label("Frente");
                ui.horizontal_wrapped(|ui| {
                    for (id, style) in FACES {
                        let mut sample = self.theme.clone();
                        sample.faces.style = *style;
                        sample.faces.directory = None;
                        if face_tile(ui, &sample, &mut self.textures, self.face_id == *id) {
                            self.face_id = (*id).to_string();
                            changed = true;
                        }
                    }
                    for deck in KDE {
                        if picture_tile(
                            ui,
                            asset(deck.preview),
                            egui::vec2(70.0, 98.0),
                            self.face_id == deck.id,
                            &mut self.textures,
                            true,
                        ) {
                            self.face_id = deck.id.to_string();
                            changed = true;
                        }
                    }
                });
                ui.add_space(10.0);
                ui.label("Dorso");
                ui.horizontal_wrapped(|ui| {
                    for id in BACKS {
                        if picture_tile(
                            ui,
                            asset(&format!("dorsos/{id}.jpg")),
                            egui::vec2(62.0, 88.0),
                            self.back_id == *id,
                            &mut self.textures,
                            true,
                        ) {
                            self.back_id = (*id).to_string();
                            changed = true;
                        }
                    }
                    for deck in KDE {
                        let back_id = format!("{}-dorso", deck.id);
                        if picture_tile(
                            ui,
                            asset(&format!("{}/back.png", deck.raster)),
                            egui::vec2(62.0, 88.0),
                            self.back_id == back_id,
                            &mut self.textures,
                            true,
                        ) {
                            self.back_id = back_id;
                            changed = true;
                        }
                    }
                });
                ui.add_space(10.0);
                ui.label("Tapete");
                ui.horizontal_wrapped(|ui| {
                    for id in FELTS {
                        if picture_tile(
                            ui,
                            asset(&format!("tapetes/{id}.jpg")),
                            egui::vec2(116.0, 66.0),
                            self.felt_id == *id,
                            &mut self.textures,
                            false,
                        ) {
                            self.felt_id = (*id).to_string();
                            self.theme.table.image = Some(asset(&format!("tapetes/{id}.jpg")));
                            changed = true;
                        }
                    }
                    });
                });
            });
        if close {
            self.show_options = false;
        }
        if changed {
            apply_look(&mut self.theme, &self.face_id, &self.back_id, &self.felt_id);
            self.textures.reload(ctx, &self.theme);
            save_opciones(self);
        }
    }

    fn games_window(&mut self, ctx: &egui::Context) {
        if !self.show_games {
            return;
        }
        let mut open = self.show_games;
        let mut chosen = None;
        egui::Window::new("Elegir juego")
            .id(egui::Id::new("ventana-juegos"))
            .open(&mut open)
            .resizable(false)
            .collapsible(false)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    for kind in [
                        GameKind::Spider1,
                        GameKind::Spider2,
                        GameKind::Spider4,
                        GameKind::FreeCell,
                        GameKind::Klondike,
                        GameKind::Hearts,
                    ] {
                        if game_tile(ui, &self.theme, &mut self.textures, kind, self.kind == kind) {
                            chosen = Some(kind);
                        }
                    }
                });
            });
        self.show_games = open;
        if let Some(kind) = chosen {
            self.choose_game(kind);
        }
    }
}

impl eframe::App for CualApp {
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        save_partida(self);
        self.textures.release();
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if ctx.input(|input| input.viewport().close_requested()) {
            save_partida(self);
        }
        if !ctx.wants_keyboard_input() {
            let (undo, redo) = ctx.input_mut(|input| {
                let undo = input.consume_shortcut(&egui::KeyboardShortcut::new(Modifiers::COMMAND, Key::Z));
                let redo = input.consume_shortcut(&egui::KeyboardShortcut::new(Modifiers::COMMAND, Key::Y))
                    || input.consume_shortcut(&egui::KeyboardShortcut::new(
                        Modifiers::COMMAND | Modifiers::SHIFT,
                        Key::Z,
                    ));
                (undo, redo)
            });
            if undo {
                self.undo();
            } else if redo {
                self.redo();
            }
        }

        self.games_window(ctx);
        self.options_panel(ctx);

        egui::TopBottomPanel::top("barra")
            .frame(egui::Frame::NONE.inner_margin(egui::Margin::symmetric(8, 6)))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        ui.add_space((ui.available_width() - 360.0).max(0.0) / 2.0);
                            if token(ui, Icon::Games, "Elegir juego", true, self.show_games) {
                                self.show_games = !self.show_games;
                            }
                            if token(ui, Icon::Shuffle, "Mezclar de nuevo", true, false) {
                                self.reshuffle();
                            }
                            if token(ui, Icon::Restart, "Reiniciar este reparto", true, false) {
                                self.restart();
                            }
                            if token(ui, Icon::Undo, "Deshacer", !self.history.undo.is_empty(), false) {
                                self.undo();
                            }
                            if token(ui, Icon::Redo, "Rehacer", !self.history.redo.is_empty(), false) {
                                self.redo();
                            }
                            if token(ui, Icon::Auto, "Jugar solo", true, self.auto) {
                                self.auto = !self.auto;
                                if self.auto {
                                    let before = snapshot(&self.table);
                                    self.launch_auto(ctx);
                                    if self.flight.is_some() {
                                        self.history.undo.push(before);
                                        self.history.redo.clear();
                                    }
                                }
                                save_opciones(self);
                            }
                            if token(ui, Icon::Themes, "Apariencia", true, self.show_options) {
                                self.show_options = !self.show_options;
                            }
                        });
                    });
            });

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ctx, |ui| {
                let rect = ui.max_rect();
                if matches!(self.table, Table::Hearts(_)) {
                    let now = ctx.input(|input| input.time);
                    draw_tablecloth(ui.painter(), rect, &self.theme, &self.textures);
                    let finished = matches!(&self.table, Table::Hearts(game) if game.finished);
                    if finished && self.win.is_none() {
                        self.win = Some(WinAnim::start(now));
                    }
                    paint_hearts(
                        ui,
                        &mut self.table,
                        &self.theme,
                        &mut self.textures,
                        now,
                        &mut self.pace,
                        &mut self.hearts_hand,
                        &mut self.hearts_deal,
                        &mut self.hearts_seen,
                        &mut self.hearts_play,
                        &mut self.hearts_gather,
                        &mut self.hearts_from,
                    );
                    if let Some(win) = &mut self.win {
                        if finished {
                            let dt = (now - win.last).clamp(0.0, 0.05) as f32;
                            win.last = now;
                            let area = rect.shrink(8.0);
                            win.step(dt, area, Vec2::new(78.0, 110.0));
                            if !win.settled() {
                                ctx.request_repaint();
                            }
                            paint_win(ui.painter(), win, &self.theme, &mut self.textures, ctx);
                        }
                    }
                    return;
                }
                let play_area = rect.shrink(8.0);
                self.play_area = play_area;
                if let Some(flight) = &self.flight {
                    let now = ctx.input(|input| input.time);
                    if now - flight.start >= flight.secs {
                        self.flight = None;
                        if self.table_won() {
                            self.win = Some(WinAnim::start(now));
                        } else {
                            self.launch_auto(ctx);
                        }
                    } else {
                        ctx.request_repaint();
                    }
                }
                let board = match &self.table {
                    Table::Spider(game) => spider_board(play_area, game),
                    Table::FreeCell(game) => freecell_board(play_area, game),
                    Table::Klondike(game) => klondike_board(play_area, game),
                    Table::Hearts(_) => empty_board(),
                };
                self.play(ctx, &board);
                let painter = ui.painter_at(rect);
                draw_tablecloth(&painter, rect, &self.theme, &self.textures);
                let hidden = self.drag.as_ref().and_then(|drag| match drag.source {
                    DragSource::Spider { column, count }
                    | DragSource::Cascade { column, count }
                    | DragSource::Klondike { column, count } => {
                        let rows = match &self.table {
                            Table::Spider(game) => game.tableau[column].len(),
                            Table::FreeCell(game) => game.cascades[column].len(),
                            Table::Klondike(game) => game.tableau[column].len(),
                            Table::Hearts(_) => 0,
                        };
                        Some(Hidden {
                            column,
                            from_row: rows.saturating_sub(count),
                        })
                    }
                    DragSource::FreeCell(_) | DragSource::Waste => None,
                });
                let hidden_cell = self.drag.as_ref().and_then(|drag| match drag.source {
                    DragSource::FreeCell(cell) => Some(cell),
                    _ => None,
                });
                match &self.table {
                    Table::Spider(game) => {
                        draw_spider(
                            &painter,
                            play_area,
                            &board,
                            game,
                            &self.theme,
                            &mut self.textures,
                            ctx,
                            hidden,
                            self.flight.as_ref().and_then(|flight| flight.conceal),
                        )
                    }
                    Table::FreeCell(game) => draw_freecell(
                        &painter,
                        &board,
                        game,
                        &self.theme,
                        &mut self.textures,
                        ctx,
                        hidden,
                        hidden_cell,
                        self.flight.as_ref().and_then(|flight| flight.conceal),
                        self.flight.as_ref().and_then(|flight| flight.foundation),
                    ),
                    Table::Klondike(game) => draw_klondike(
                        &painter,
                        &board,
                        game,
                        &self.theme,
                        &mut self.textures,
                        ctx,
                        hidden,
                        self.flight.as_ref().and_then(|flight| flight.conceal),
                        self.flight.as_ref().and_then(|flight| flight.foundation),
                        self.flight.as_ref().map(|flight| flight.waste_hide).unwrap_or(0),
                    ),
                    Table::Hearts(_) => {}
                }
                if let Some(flight) = &self.flight {
                    let now = ctx.input(|input| input.time);
                    for (index, card) in flight.cards.iter().enumerate() {
                        let depart = flight.start + index as f64 * flight.stagger;
                        if now < depart {
                            continue;
                        }
                        let t = ((now - depart) / FLIGHT_SECS).clamp(0.0, 1.0) as f32;
                        let eased = t * t * (3.0 - 2.0 * t);
                        let from = flight.from.get(index).or(flight.from.last());
                        let to = flight.to.get(index).or(flight.to.last());
                        if let (Some(from), Some(to)) = (from, to) {
                            let origin = from.min + (to.min - from.min) * eased;
                            let rect = Rect::from_min_size(origin, from.size());
                            draw_face(&painter, rect, *card, &self.theme, &mut self.textures, ctx, true);
                        }
                    }
                }
                if let Some(win) = &mut self.win {
                    let now = ctx.input(|input| input.time);
                    let dt = (now - win.last).clamp(0.0, 0.05) as f32;
                    win.last = now;
                    win.step(dt, play_area, board.card);
                    if !win.settled() {
                        ctx.request_repaint();
                    }
                    let card_size = board.card * 0.72;
                    for piece in &win.pieces {
                        for (age, (x, y)) in piece.trail.iter().rev().enumerate() {
                            if age == 0 {
                                continue;
                            }
                            let scale = 1.0 - age as f32 * 0.04;
                            let rect = Rect::from_min_size(Pos2::new(*x, *y), card_size * scale);
                            draw_face(&painter, rect, piece.card, &self.theme, &mut self.textures, ctx, true);
                        }
                        let rect = Rect::from_min_size(Pos2::new(piece.x, piece.y), card_size);
                        draw_face(&painter, rect, piece.card, &self.theme, &mut self.textures, ctx, true);
                    }
                }
                if let Some(drag) = &self.drag {
                    if let Some(rect) = drop_highlight(&self.table, &board, drag, drag.pointer) {
                        highlight(&painter, rect);
                    }
                }
            });

        if matches!(&self.table, Table::Hearts(game) if game.finished)
            && self.win.as_ref().is_some_and(|win| win.settled())
        {
            let scores = match &self.table {
                Table::Hearts(game) => game.scores,
                _ => [0; 4],
            };
            let mut another = false;
            let mut again = false;
            egui::Area::new(egui::Id::new("victoria"))
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                .interactable(true)
                .show(ctx, |ui| {
                    egui::Frame::popup(ui.style())
                        .fill(egui::Color32::from_rgba_unmultiplied(12, 18, 16, 230))
                        .inner_margin(egui::Margin::same(16))
                        .show(ui, |ui| {
                            ui.set_min_width(280.0);
                            ui.vertical_centered(|ui| {
                                ui.label(egui::RichText::new(hearts_title(scores)).size(22.0).strong());
                                ui.add_space(8.0);
                            });
                            for (place, seat, score) in hearts_ranking(scores) {
                                ui.label(
                                    egui::RichText::new(format!(
                                        "{place}.º   {}   {score}",
                                        cualsol_hearts::seat_name(seat)
                                    ))
                                    .size(18.0),
                                );
                            }
                            ui.add_space(10.0);
                            ui.horizontal(|ui| {
                                if token(ui, Icon::Games, "Elegir otro juego", true, false) {
                                    another = true;
                                }
                                if token(ui, Icon::Shuffle, "Mezclar de nuevo", true, false) {
                                    again = true;
                                }
                            });
                        });
                });
            if another {
                self.win = None;
                self.show_games = true;
            }
            if again {
                self.reshuffle();
            }
        } else if self.win.as_ref().is_some_and(|win| win.settled()) {
            let mut another = false;
            let mut again = false;
            egui::Area::new(egui::Id::new("victoria"))
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                .interactable(true)
                .show(ctx, |ui| {
                    egui::Frame::popup(ui.style())
                        .fill(egui::Color32::from_rgba_unmultiplied(12, 18, 16, 220))
                        .inner_margin(egui::Margin::same(16))
                        .show(ui, |ui| {
                            ui.vertical_centered(|ui| {
                                ui.label(egui::RichText::new("Ganaste").size(22.0).strong());
                                ui.add_space(8.0);
                                ui.horizontal(|ui| {
                                    if token(ui, Icon::Games, "Elegir otro juego", true, false) {
                                        another = true;
                                    }
                                    if token(ui, Icon::Shuffle, "Mezclar de nuevo", true, false) {
                                        again = true;
                                    }
                                });
                            });
                        });
                });
            if another {
                self.win = None;
                self.show_games = true;
            }
            if again {
                self.reshuffle();
            }
        }

        if let Some(drag) = &self.drag {
            let layer = egui::LayerId::new(egui::Order::Tooltip, egui::Id::new("carta-arrastrada"));
            let painter = ctx.layer_painter(layer);
            for (index, card) in drag.cards.iter().enumerate() {
                let origin = drag.pointer - drag.offset + Vec2::new(0.0, index as f32 * drag.size.y * FAN);
                let rect = Rect::from_min_size(origin, drag.size);
                draw_face(&painter, rect, *card, &self.theme, &mut self.textures, ctx, true);
            }
            ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
        }
    }
}

impl CualApp {
    fn play(&mut self, ctx: &egui::Context, board: &Board) {
        let pointer = ctx.input(|input| input.pointer.hover_pos().or(input.pointer.interact_pos()));
        let pressed = ctx.input(|input| input.pointer.primary_pressed());
        let released = ctx.input(|input| input.pointer.primary_released());
        let over_ui = ctx.wants_pointer_input()
            || pointer.is_some_and(|pos| blocks_table(ctx, pos));
        if self.flight.is_some() || self.win.is_some() {
            return;
        }
        let secondary = ctx.input(|input| input.pointer.secondary_clicked());
        if secondary && !over_ui && self.drag.is_none() {
            if let Some(pos) = pointer {
                self.quick_move(board, pos, ctx);
            }
            return;
        }

        if let Some(drag) = &mut self.drag {
            if let Some(pos) = pointer.or(ctx.input(|input| input.pointer.interact_pos())) {
                drag.pointer = pos;
            }
        }

        if pressed && !over_ui && self.drag.is_none() {
            if let Some(pos) = pointer {
                self.press = hit(board, pos).map(|hit| Press { pos, hit });
                self.begin_drag(board, pos);
            }
        }

        if released {
            let pos = pointer.or(ctx.input(|input| input.pointer.interact_pos()));
            if self.drag.is_some() {
                if let Some(pos) = pos {
                    self.drop_at(board, pos, ctx);
                } else {
                    self.drag = None;
                }
            } else if let (Some(press), Some(pos)) = (&self.press, pos) {
                let moved = press.pos.distance(pos) < 6.0;
                if moved && matches!(press.hit, GeomHit::Extra(ExtraKind::Stock)) {
                    if matches!(hit(board, pos), Some(GeomHit::Extra(ExtraKind::Stock))) {
                        self.deal_stock(ctx);
                    }
                }
            }
            self.press = None;
        }
    }

    fn quick_move(&mut self, board: &Board, pos: Pos2, ctx: &egui::Context) {
        if matches!(hit(board, pos), Some(GeomHit::Extra(ExtraKind::Stock))) {
            self.deal_stock(ctx);
            return;
        }
        let before = snapshot(&self.table);
        let moved = match (&self.table, hit(board, pos)) {
            (Table::Spider(game), Some(GeomHit::Card { column, row })) => {
                spider_quick(game, column, row).is_some_and(|movement| match movement {
                    SpiderMove::Transfer { from, count, to } => self.glide_spider(ctx, from, count, to),
                    SpiderMove::DealStock => false,
                })
            }
            (Table::FreeCell(game), Some(GeomHit::Card { column, row })) => freecell_quick_cascade(game, column, row)
                .is_some_and(|movement| self.glide_free(ctx, movement)),
            (Table::FreeCell(game), Some(GeomHit::Extra(ExtraKind::FreeCell(cell)))) => {
                freecell_quick_cell(game, cell).is_some_and(|movement| self.glide_free(ctx, movement))
            }
            (Table::Klondike(game), Some(GeomHit::Card { column, row })) => {
                klondike_quick_column(game, column, row).is_some_and(|movement| self.glide_klondike(ctx, movement))
            }
            (Table::Klondike(game), Some(GeomHit::Extra(ExtraKind::Waste))) => {
                klondike_quick_waste(game).is_some_and(|movement| self.glide_klondike(ctx, movement))
            }
            _ => false,
        };
        if moved {
            self.history.undo.push(before);
            self.history.redo.clear();
            self.status.clear();
        }
    }

    fn table_won(&self) -> bool {
        match &self.table {
            Table::Spider(game) => game.won(),
            Table::FreeCell(game) => game.won(),
            Table::Klondike(game) => game.won(),
            Table::Hearts(_) => false,
        }
    }
}

fn paint_hearts(
    ui: &mut egui::Ui,
    table: &mut Table,
    theme: &Theme,
    textures: &mut Textures,
    now: f64,
    pace: &mut f64,
    hearts_hand: &mut u32,
    hearts_deal: &mut f64,
    hearts_seen: &mut usize,
    hearts_play: &mut f64,
    hearts_gather: &mut f64,
    hearts_from: &mut Rect,
) {
    let Table::Hearts(game) = table else {
        return;
    };
    hearts_view::show(
        ui,
        game,
        theme,
        textures,
        now,
        pace,
        hearts_hand,
        hearts_deal,
        hearts_seen,
        hearts_play,
        hearts_gather,
        hearts_from,
    );
}

fn paint_win(painter: &egui::Painter, win: &WinAnim, theme: &Theme, textures: &mut Textures, ctx: &egui::Context) {
    let card_size = Vec2::new(78.0, 110.0) * 0.72;
    for piece in &win.pieces {
        for (age, (x, y)) in piece.trail.iter().rev().enumerate() {
            if age == 0 {
                continue;
            }
            let scale = 1.0 - age as f32 * 0.04;
            let rect = Rect::from_min_size(Pos2::new(*x, *y), card_size * scale);
            draw_face(painter, rect, piece.card, theme, textures, ctx, true);
        }
        let rect = Rect::from_min_size(Pos2::new(piece.x, piece.y), card_size);
        draw_face(painter, rect, piece.card, theme, textures, ctx, true);
    }
}

fn hearts_ranking(scores: [i32; 4]) -> Vec<(usize, usize, i32)> {
    let mut order: Vec<usize> = (0..4).collect();
    order.sort_by_key(|seat| (scores[*seat], *seat));
    let mut rows: Vec<(usize, usize, i32)> = Vec::with_capacity(4);
    for (index, seat) in order.iter().copied().enumerate() {
        let place = if index > 0 && scores[seat] == scores[order[index - 1]] {
            rows[index - 1].0
        } else {
            index + 1
        };
        rows.push((place, seat, scores[seat]));
    }
    rows
}

fn hearts_title(scores: [i32; 4]) -> String {
    let winners: Vec<usize> = hearts_ranking(scores)
        .into_iter()
        .filter(|(place, _, _)| *place == 1)
        .map(|(_, seat, _)| seat)
        .collect();
    if winners == [cualsol_hearts::HUMAN] {
        "Ganaste".to_string()
    } else if winners.contains(&cualsol_hearts::HUMAN) {
        "Empataste el primer puesto".to_string()
    } else if let [seat] = winners.as_slice() {
        format!("Ganó {}", cualsol_hearts::seat_name(*seat))
    } else {
        "Empate".to_string()
    }
}

fn empty_board() -> Board {
    Board {
        card: Vec2::new(72.0, 100.0),
        piles: Vec::new(),
        extras: Vec::new(),
    }
}

fn drop_highlight(table: &Table, board: &Board, drag: &Drag, pos: Pos2) -> Option<Rect> {
    match (table, &drag.source) {
        (Table::Spider(game), DragSource::Spider { column, count }) => {
            let to = column_under(board, pos)?;
            if to == *column {
                return None;
            }
            let mut trial = game.clone();
            trial
                .apply(SpiderMove::Transfer {
                    from: *column,
                    count: *count,
                    to,
                })
                .ok()?;
            Some(pile_rect(board, to))
        }
        (Table::FreeCell(game), DragSource::Cascade { column, count }) => {
            if let Some(ExtraKind::FreeCell(cell)) = extra_at(board, pos) {
                if *count == 1 && game.freecells[cell].is_none() {
                    return board.extras.iter().find(|extra| extra.kind == ExtraKind::FreeCell(cell)).map(|extra| extra.rect);
                }
            }
            if let Some(ExtraKind::Foundation(index)) = extra_at(board, pos) {
                if *count == 1 {
                    let card = *drag.cards.last()?;
                    if game.accepts_foundation(card) && foundation_index(card) == index {
                        return board
                            .extras
                            .iter()
                            .find(|extra| extra.kind == ExtraKind::Foundation(index))
                            .map(|extra| extra.rect);
                    }
                }
            }
            let to = column_under(board, pos)?;
            if to == *column {
                return None;
            }
            let mut trial = game.clone();
            trial
                .apply(FreeMove::Run {
                    from: *column,
                    count: *count,
                    to,
                })
                .ok()?;
            Some(pile_rect(board, to))
        }
        (Table::FreeCell(game), DragSource::FreeCell(cell)) => {
            if let Some(kind) = extra_at(board, pos) {
                return match kind {
                    ExtraKind::Foundation(index) => {
                        let card = game.freecells[*cell]?;
                        if game.accepts_foundation(card) && foundation_index(card) == index {
                            board
                                .extras
                                .iter()
                                .find(|extra| extra.kind == ExtraKind::Foundation(index))
                                .map(|extra| extra.rect)
                        } else {
                            None
                        }
                    }
                    ExtraKind::FreeCell(to) if to != *cell && game.freecells[to].is_none() => board
                        .extras
                        .iter()
                        .find(|extra| extra.kind == ExtraKind::FreeCell(to))
                        .map(|extra| extra.rect),
                    _ => None,
                };
            }
            let to = column_under(board, pos)?;
            let mut trial = game.clone();
            trial
                .apply(FreeMove::FreecellToCascade { cell: *cell, to })
                .ok()?;
            Some(pile_rect(board, to))
        }
        _ => None,
    }
}

fn extra_rect(board: &Board, kind: ExtraKind) -> Option<Rect> {
    board.extras.iter().find(|extra| extra.kind == kind).map(|extra| extra.rect)
}

fn drop_klondike_run(game: &mut Klondike, board: &Board, pos: Pos2, column: usize, count: usize) -> bool {
    if count == 1 {
        if let Some(ExtraKind::Foundation(_)) = extra_at(board, pos) {
            return game.apply(KMove::TableauToFoundation { from: column }).is_ok();
        }
    }
    let Some(to) = column_under(board, pos) else {
        return false;
    };
    game.apply(KMove::Tableau { from: column, count, to }).is_ok()
}

fn drop_klondike_waste(game: &mut Klondike, board: &Board, pos: Pos2) -> bool {
    if let Some(ExtraKind::Foundation(_)) = extra_at(board, pos) {
        return game.apply(KMove::WasteToFoundation).is_ok();
    }
    let Some(to) = column_under(board, pos) else {
        return false;
    };
    game.apply(KMove::WasteToTableau { to }).is_ok()
}

fn klondike_quick_column(game: &Klondike, column: usize, row: usize) -> Option<KMove> {
    let count = game.liftable_from(column, row)?;
    if count == 1 && game.tableau[column].last().is_some_and(|placed| game.accepts_foundation(placed.card)) {
        return Some(KMove::TableauToFoundation { from: column });
    }
    for to in 0..cualsol_klondike::COLUMNS {
        if to == column {
            continue;
        }
        let mut trial = game.clone();
        if trial.apply(KMove::Tableau { from: column, count, to }).is_ok() {
            return Some(KMove::Tableau { from: column, count, to });
        }
    }
    None
}

fn klondike_quick_waste(game: &Klondike) -> Option<KMove> {
    let card = game.waste.last().copied()?;
    if game.accepts_foundation(card) {
        return Some(KMove::WasteToFoundation);
    }
    for to in 0..cualsol_klondike::COLUMNS {
        let mut trial = game.clone();
        if trial.apply(KMove::WasteToTableau { to }).is_ok() {
            return Some(KMove::WasteToTableau { to });
        }
    }
    None
}

fn pile_rect(board: &Board, column: usize) -> Rect {
    let pile = board
        .piles
        .iter()
        .find(|pile| pile.index == column)
        .expect("columna");
    pile.cards.last().copied().unwrap_or(pile.slot)
}

fn column_under(board: &Board, pos: Pos2) -> Option<usize> {
    if matches!(
        extra_at(board, pos),
        Some(ExtraKind::FreeCell(_) | ExtraKind::Foundation(_) | ExtraKind::Stock | ExtraKind::Waste)
    ) {
        return None;
    }
    crate::paint::column_at(board, pos)
}

fn drop_freecell_from_cascade(
    game: &mut FreeCell,
    board: &Board,
    pos: Pos2,
    column: usize,
    count: usize,
) -> bool {
    if let Some(ExtraKind::FreeCell(cell)) = extra_at(board, pos) {
        return count == 1
            && game
                .apply(FreeMove::CascadeToFreecell { from: column, cell })
                .is_ok();
    }
    if let Some(ExtraKind::Foundation(_)) = extra_at(board, pos) {
        return count == 1
            && game
                .apply(FreeMove::ToFoundation {
                    from: Source::Cascade(column),
                })
                .is_ok();
    }
    let Some(to) = column_under(board, pos) else {
        return false;
    };
    if to == column {
        return false;
    }
    game.apply(FreeMove::Run {
        from: column,
        count,
        to,
    })
    .is_ok()
}

fn drop_freecell_from_cell(game: &mut FreeCell, board: &Board, pos: Pos2, cell: usize) -> bool {
    match extra_at(board, pos) {
        Some(ExtraKind::Foundation(_)) => game
            .apply(FreeMove::ToFoundation {
                from: Source::Freecell(cell),
            })
            .is_ok(),
        Some(ExtraKind::FreeCell(to)) => game
            .apply(FreeMove::FreecellToFreecell { from: cell, to })
            .is_ok(),
        _ => {
            let Some(to) = column_under(board, pos) else {
                return false;
            };
            game.apply(FreeMove::FreecellToCascade { cell, to }).is_ok()
        }
    }
}

fn foundation_index(card: Card) -> usize {
    Suit::ALL.iter().position(|suit| *suit == card.suit).unwrap_or(0)
}

fn snapshot(table: &Table) -> Table {
    match table {
        Table::Spider(game) => Table::Spider(game.clone()),
        Table::FreeCell(game) => Table::FreeCell(game.clone()),
        Table::Klondike(game) => Table::Klondike(game.clone()),
        Table::Hearts(game) => Table::Hearts(game.clone()),
    }
}

const FACES: &[(&str, FaceStyle)] = &[
    ("clasico", FaceStyle::Classic),
    ("grande", FaceStyle::Grande),
    ("cuatro", FaceStyle::FourColor),
    ("noche", FaceStyle::Night),
];

struct KdeDeck {
    id: &'static str,
    raster: &'static str,
    preview: &'static str,
}

const KDE: &[KdeDeck] = &[
    KdeDeck {
        id: "kde-standard",
        raster: "carddecks/standard/raster",
        preview: "carddecks/standard/11.png",
    },
    KdeDeck {
        id: "kde-xskat-frances",
        raster: "carddecks/xskat-french/raster",
        preview: "carddecks/xskat-french/11.png",
    },
    KdeDeck {
        id: "kde-xskat-aleman",
        raster: "carddecks/xskat-german/raster",
        preview: "carddecks/xskat-german/11.png",
    },
    KdeDeck {
        id: "kde-pinguinos",
        raster: "carddecks/penguins/raster",
        preview: "carddecks/penguins/11.png",
    },
    KdeDeck {
        id: "kde-tigullio",
        raster: "carddecks/tigullio/raster",
        preview: "carddecks/tigullio/queen-of-hearts.png",
    },
    KdeDeck {
        id: "kde-nicu-ornamental",
        raster: "carddecks/nicu-ornamental/raster",
        preview: "carddecks/nicu-ornamental/11.png",
    },
    KdeDeck {
        id: "kde-nicu-blanco",
        raster: "carddecks/nicu-white/raster",
        preview: "carddecks/nicu-white/11.png",
    },
];

fn kde_by_id(id: &str) -> Option<&'static KdeDeck> {
    KDE.iter().find(|deck| deck.id == id)
}
const BACKS: &[&str] = &["dorso1", "dorso2", "dorso3", "dorso4"];
const FELTS: &[&str] = &["tapete1", "tapete2", "tapete3", "tapete4"];

fn fresh_seed() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos() as u64)
        .unwrap_or(1)
}

fn face_tile(ui: &mut egui::Ui, theme: &Theme, textures: &mut Textures, selected: bool) -> bool {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(70.0, 98.0), egui::Sense::click());
    draw_face(
        ui.painter(),
        rect,
        Card::new(Suit::Hearts, Rank::Ace),
        theme,
        textures,
        ui.ctx(),
        false,
    );
    if selected || response.hovered() {
        highlight(ui.painter(), rect);
    }
    ui.add_space(6.0);
    response.clicked()
}

fn picture_tile(
    ui: &mut egui::Ui,
    path: PathBuf,
    size: egui::Vec2,
    selected: bool,
    textures: &mut Textures,
    round: bool,
) -> bool {
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    if let Some(texture) = textures.texture(&path) {
        ui.painter().image(
            texture.id(),
            rect,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            egui::Color32::WHITE,
        );
    } else if round {
        draw_back(ui.painter(), rect, &Theme::builtin(), textures, ui.ctx());
    } else {
        ui.painter().rect_filled(rect, 6.0, egui::Color32::from_rgb(20, 80, 50));
    }
    if selected || response.hovered() {
        highlight(ui.painter(), rect);
    }
    ui.add_space(8.0);
    response.clicked()
}

fn game_tile(
    ui: &mut egui::Ui,
    theme: &Theme,
    textures: &mut Textures,
    kind: GameKind,
    selected: bool,
) -> bool {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(108.0, 132.0), egui::Sense::click());
    ui.painter()
        .rect_filled(rect, 8.0, egui::Color32::from_rgba_unmultiplied(10, 24, 18, 180));
    let card = egui::vec2(36.0, 50.0);
    match kind {
        GameKind::FreeCell => {
            for index in 0..4 {
                let slot = Rect::from_min_size(
                    rect.left_top() + egui::vec2(8.0 + index as f32 * 24.0, 16.0),
                    egui::vec2(20.0, 28.0),
                );
                ui.painter().rect_stroke(
                    slot,
                    3.0,
                    egui::Stroke::new(1.0_f32, egui::Color32::from_white_alpha(140)),
                    egui::StrokeKind::Inside,
                );
            }
            draw_face(
                ui.painter(),
                Rect::from_min_size(rect.left_top() + egui::vec2(18.0, 58.0), card),
                Card::new(Suit::Spades, Rank::King),
                theme,
                textures,
                ui.ctx(),
                true,
            );
            draw_face(
                ui.painter(),
                Rect::from_min_size(rect.left_top() + egui::vec2(48.0, 70.0), card),
                Card::new(Suit::Hearts, Rank::Queen),
                theme,
                textures,
                ui.ctx(),
                true,
            );
        }
        GameKind::Hearts => {
            draw_face(
                ui.painter(),
                Rect::from_min_size(rect.left_top() + egui::vec2(36.0, 40.0), card),
                Card::new(Suit::Hearts, Rank::Queen),
                theme,
                textures,
                ui.ctx(),
                true,
            );
        }
        GameKind::Klondike => {
            let card = egui::vec2(36.0, 50.0);
            for (index, rank) in [Rank::King, Rank::Queen, Rank::Jack].into_iter().enumerate() {
                draw_face(
                    ui.painter(),
                    Rect::from_min_size(rect.left_top() + egui::vec2(18.0 + index as f32 * 14.0, 36.0), card),
                    Card::new(Suit::Spades, rank),
                    theme,
                    textures,
                    ui.ctx(),
                    true,
                );
            }
        }
        GameKind::Spider1 | GameKind::Spider2 | GameKind::Spider4 => {
            let suits = match kind {
                GameKind::Spider1 => vec![Suit::Spades],
                GameKind::Spider2 => vec![Suit::Spades, Suit::Hearts],
                _ => Suit::ALL.to_vec(),
            };
            for (index, suit) in suits.iter().enumerate() {
                draw_face(
                    ui.painter(),
                    Rect::from_min_size(
                        rect.left_top() + egui::vec2(14.0 + index as f32 * 16.0, 28.0 + index as f32 * 8.0),
                        card,
                    ),
                    Card::new(*suit, Rank::Ace),
                    theme,
                    textures,
                    ui.ctx(),
                    true,
                );
            }
        }
    }
    response.clone().on_hover_text(kind.name());
    if selected || response.hovered() {
        highlight(ui.painter(), rect);
    }
    ui.add_space(8.0);
    response.clicked()
}

fn blocks_table(ctx: &egui::Context, pos: Pos2) -> bool {
    const IDS: &[&str] = &["ventana-opciones", "ventana-juegos", "barra", "victoria"];
    ctx.memory(|memory| {
        IDS.iter().any(|id| {
            memory
                .area_rect(egui::Id::new(*id))
                .is_some_and(|rect| rect.contains(pos))
        })
    })
}

fn open_default_theme() -> Theme {
    let path = default_theme_path();
    load_theme(&path).unwrap_or_else(|_| Theme::builtin())
}

fn default_theme_path() -> PathBuf {
    let from_cwd = PathBuf::from("themes/clasico/theme.toml");
    if from_cwd.is_file() {
        return from_cwd;
    }
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../themes/clasico/theme.toml")
}

fn completed_rects(area: Rect, card: Vec2, count: usize) -> Vec<Rect> {
    let mini = Vec2::new(card.x * 0.62, card.y * 0.62);
    let origin = Pos2::new(area.left() + 8.0, area.bottom() - mini.y - 8.0);
    (0..count)
        .map(|index| Rect::from_min_size(origin + Vec2::new(index as f32 * 6.0, 0.0), mini))
        .collect()
}

fn spider_quick(game: &Spider, column: usize, row: usize) -> Option<SpiderMove> {
    let count = game.liftable_from(column, row)?;
    let mut target = None;
    for to in 0..cualsol_spider::COLUMNS {
        if to == column {
            continue;
        }
        let mut trial = game.clone();
        if trial
            .apply(SpiderMove::Transfer {
                from: column,
                count,
                to,
            })
            .is_ok()
        {
            target = Some(to);
            if !game.tableau[to].is_empty() {
                break;
            }
        }
    }
    target.map(|to| SpiderMove::Transfer {
        from: column,
        count,
        to,
    })
}

fn freecell_quick_cascade(game: &FreeCell, column: usize, row: usize) -> Option<FreeMove> {
    let count = game.liftable_from(column, row)?;
    if count == 1 && game.cascades[column].last().is_some_and(|placed| game.accepts_foundation(placed.card)) {
        return Some(FreeMove::ToFoundation {
            from: Source::Cascade(column),
        });
    }
    for to in 0..cualsol_freecell::CASCADES {
        if to == column {
            continue;
        }
        let mut trial = game.clone();
        if trial
            .apply(FreeMove::Run {
                from: column,
                count,
                to,
            })
            .is_ok()
        {
            return Some(FreeMove::Run {
                from: column,
                count,
                to,
            });
        }
    }
    if count == 1 {
        if let Some(cell) = game.freecells.iter().position(|cell| cell.is_none()) {
            return Some(FreeMove::CascadeToFreecell { from: column, cell });
        }
    }
    None
}

fn freecell_quick_cell(game: &FreeCell, cell: usize) -> Option<FreeMove> {
    if game.freecells[cell].is_some_and(|card| game.accepts_foundation(card)) {
        return Some(FreeMove::ToFoundation {
            from: Source::Freecell(cell),
        });
    }
    for to in 0..cualsol_freecell::CASCADES {
        let mut trial = game.clone();
        if trial.apply(FreeMove::FreecellToCascade { cell, to }).is_ok() {
            return Some(FreeMove::FreecellToCascade { cell, to });
        }
    }
    None
}

impl WinAnim {
    fn start(now: f64) -> Self {
        Self {
            last: now,
            next_spawn: now,
            spawned: 0,
            pieces: Vec::new(),
        }
    }

    fn settled(&self) -> bool {
        self.spawned >= 52 && self.pieces.iter().all(|piece| piece.rest)
    }

    fn step(&mut self, dt: f32, area: Rect, card: Vec2) {
        let gravity = 2100.0;
        let width = card.x * 0.72;
        let height = card.y * 0.72;
        let floor = area.bottom() - height - 6.0;
        if self.spawned < 52 {
            self.next_spawn -= dt as f64;
            if self.next_spawn <= self.last {
                let index = self.spawned;
                let suit = Suit::ALL[index % 4];
                let rank = Rank::ALL[index % 13];
                let x = area.left() + 16.0 + (index as f32 * 37.0) % (area.width() - width - 32.0).max(1.0);
                self.pieces.push(Piece {
                    card: Card::new(suit, rank),
                    x,
                    y: area.top() - height,
                    vx: ((index as f32 * 1.7).sin()) * 90.0,
                    vy: 80.0 + (index % 5) as f32 * 20.0,
                    trail: Vec::new(),
                    rest: false,
                });
                self.spawned += 1;
                self.next_spawn = self.last + 0.045;
            }
        }
        for piece in &mut self.pieces {
            if piece.rest {
                continue;
            }
            piece.trail.push((piece.x, piece.y));
            if piece.trail.len() > 7 {
                piece.trail.remove(0);
            }
            piece.vy += gravity * dt;
            piece.x += piece.vx * dt;
            piece.y += piece.vy * dt;
            if piece.x < area.left() {
                piece.x = area.left();
                piece.vx = piece.vx.abs() * 0.6;
            }
            if piece.x > area.right() - width {
                piece.x = area.right() - width;
                piece.vx = -piece.vx.abs() * 0.6;
            }
            if piece.y >= floor {
                piece.y = floor;
                piece.vy = -piece.vy * 0.52;
                piece.vx *= 0.86;
                if piece.vy.abs() < 80.0 {
                    piece.vy = 0.0;
                    piece.vx = 0.0;
                    piece.rest = true;
                    piece.trail.clear();
                }
            }
        }
    }
}

#[derive(Serialize, Deserialize)]
enum Partida {
    Spider(Spider),
    FreeCell(FreeCell),
    Klondike(Klondike),
    Hearts(Hearts),
}

fn partida_path() -> PathBuf {
    opciones_path().with_file_name("partida.toml")
}

fn save_partida(app: &CualApp) {
    let partida = match &app.table {
        Table::Spider(game) => Partida::Spider(game.clone()),
        Table::FreeCell(game) => Partida::FreeCell(game.clone()),
        Table::Klondike(game) => Partida::Klondike(game.clone()),
        Table::Hearts(game) => Partida::Hearts(game.clone()),
    };
    if let Ok(text) = toml::to_string(&partida) {
        let _ = std::fs::write(partida_path(), text);
    }
}

fn load_partida() -> Option<Table> {
    let text = std::fs::read_to_string(partida_path()).ok()?;
    match toml::from_str::<Partida>(&text).ok()? {
        Partida::Spider(game) => Some(Table::Spider(game)),
        Partida::FreeCell(game) => Some(Table::FreeCell(game)),
        Partida::Klondike(game) => Some(Table::Klondike(game)),
        Partida::Hearts(game) => Some(Table::Hearts(game)),
    }
}

fn opciones_path() -> PathBuf {
    if Path::new("themes/clasico/theme.toml").is_file() {
        PathBuf::from("opciones.toml")
    } else {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../opciones.toml")
    }
}

fn load_opciones() -> Opciones {
    let Ok(text) = std::fs::read_to_string(opciones_path()) else {
        return Opciones::default();
    };
    toml::from_str(&text).unwrap_or_default()
}

fn asset(relative: &str) -> PathBuf {
    let from_cwd = PathBuf::from("assets").join(relative);
    if from_cwd.is_file() {
        return from_cwd;
    }
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets")
        .join(relative)
}

fn apply_look(theme: &mut Theme, face: &str, back: &str, felt: &str) {
    theme.faces.style = FACES
        .iter()
        .find(|(id, _)| *id == face)
        .map(|(_, style)| *style)
        .unwrap_or(FaceStyle::Classic);
    if let Some(deck) = kde_by_id(face) {
        theme.faces.directory = Some(asset(deck.raster));
    } else {
        theme.faces.directory = None;
    }
    if let Some(id) = back.strip_suffix("-dorso").filter(|id| kde_by_id(id).is_some()) {
        let deck = kde_by_id(id).expect("mazo");
        theme.back.image = Some(asset(&format!("{}/back.png", deck.raster)));
    } else if BACKS.contains(&back) {
        theme.back.image = Some(asset(&format!("dorsos/{back}.jpg")));
    } else {
        theme.back.image = None;
    }
    let felt_path = asset(&format!("tapetes/{felt}.jpg"));
    if felt_path.is_file() {
        theme.table.image = Some(felt_path);
    }
}

fn save_opciones(app: &CualApp) {
    let opciones = Opciones {
        game: Some(app.kind.id().to_string()),
        face: Some(app.face_id.clone()),
        back: Some(app.back_id.clone()),
        felt: Some(app.felt_id.clone()),
        auto: app.auto,
    };
    if let Ok(text) = toml::to_string_pretty(&opciones) {
        let _ = std::fs::write(opciones_path(), text);
    }
}

fn window_icon() -> std::sync::Arc<egui::IconData> {
    let image = image::load_from_memory(include_bytes!("../../../assets/icono.jpg"))
        .expect("icono")
        .into_rgba8();
    let (width, height) = image.dimensions();
    std::sync::Arc::new(egui::IconData {
        rgba: image.into_raw(),
        width,
        height,
    })
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1180.0, 780.0])
            .with_title("Cual Solitaire")
            .with_icon(window_icon()),
        ..Default::default()
    };
    eframe::run_native(
        "Cual Solitaire",
        options,
        Box::new(|cc| Ok(Box::new(CualApp::new(cc)))),
    )
}
