use cualsol_engine::{shuffle, Card, Rank, Rng, Suit};

pub const COLUMNS: usize = 7;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Placed {
    pub card: Card,
    pub face_up: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Game {
    pub seed: u64,
    pub tableau: [Vec<Placed>; COLUMNS],
    pub stock: Vec<Card>,
    pub waste: Vec<Card>,
    pub foundations: [Option<Card>; 4],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Move {
    Draw,
    Recycle,
    WasteToTableau { to: usize },
    WasteToFoundation,
    Tableau { from: usize, count: usize, to: usize },
    TableauToFoundation { from: usize },
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum Illegal {
    #[error("movimiento ilegal")]
    Illegal,
}

impl Game {
    pub fn deal(seed: u64) -> Self {
        let mut cards = Vec::with_capacity(52);
        for suit in Suit::ALL {
            for rank in Rank::ALL {
                cards.push(Card::new(suit, rank));
            }
        }
        let mut rng = Rng::from_seed(seed);
        shuffle(&mut cards, &mut rng);
        let mut tableau: [Vec<Placed>; COLUMNS] = Default::default();
        let mut index = 0;
        for column in 0..COLUMNS {
            for row in 0..=column {
                tableau[column].push(Placed {
                    card: cards[index],
                    face_up: row == column,
                });
                index += 1;
            }
        }
        Self {
            seed,
            tableau,
            stock: cards[index..].to_vec(),
            waste: Vec::new(),
            foundations: [None; 4],
        }
    }

    pub fn apply(&mut self, movement: Move) -> Result<(), Illegal> {
        match movement {
            Move::Draw => self.draw(),
            Move::Recycle => self.recycle(),
            Move::WasteToTableau { to } => self.waste_to(to),
            Move::WasteToFoundation => self.waste_found(),
            Move::Tableau { from, count, to } => self.transfer(from, count, to),
            Move::TableauToFoundation { from } => self.column_found(from),
        }
    }

    pub fn won(&self) -> bool {
        self.foundations.iter().all(|top| top.is_some_and(|card| card.rank == Rank::King))
    }

    pub fn liftable_from(&self, column: usize, row: usize) -> Option<usize> {
        let pile = self.tableau.get(column)?;
        let tail = pile.get(row..)?;
        if tail.is_empty() || tail.iter().any(|placed| !placed.face_up) || !alternating(tail) {
            return None;
        }
        Some(tail.len())
    }

    pub fn accepts_foundation(&self, card: Card) -> bool {
        fits_foundation(card, &self.foundations)
    }

    pub fn next_foundation(&self) -> Option<Move> {
        if self.waste.last().is_some_and(|card| self.accepts_foundation(*card)) {
            return Some(Move::WasteToFoundation);
        }
        for column in 0..COLUMNS {
            if self.tableau[column]
                .last()
                .is_some_and(|placed| placed.face_up && self.accepts_foundation(placed.card))
            {
                return Some(Move::TableauToFoundation { from: column });
            }
        }
        None
    }

    fn draw(&mut self) -> Result<(), Illegal> {
        if self.stock.is_empty() {
            return Err(Illegal::Illegal);
        }
        for _ in 0..3 {
            let Some(card) = self.stock.pop() else {
                break;
            };
            self.waste.push(card);
        }
        Ok(())
    }

    fn recycle(&mut self) -> Result<(), Illegal> {
        if !self.stock.is_empty() || self.waste.is_empty() {
            return Err(Illegal::Illegal);
        }
        self.stock = self.waste.drain(..).rev().collect();
        Ok(())
    }

    fn waste_to(&mut self, to: usize) -> Result<(), Illegal> {
        let card = self.waste.last().copied().ok_or(Illegal::Illegal)?;
        if !can_build(card, self.tableau.get(to).ok_or(Illegal::Illegal)?.last().map(|placed| placed.card)) {
            return Err(Illegal::Illegal);
        }
        self.waste.pop();
        self.tableau[to].push(Placed { card, face_up: true });
        Ok(())
    }

    fn waste_found(&mut self) -> Result<(), Illegal> {
        let card = self.waste.last().copied().ok_or(Illegal::Illegal)?;
        if !fits_foundation(card, &self.foundations) {
            return Err(Illegal::Illegal);
        }
        self.waste.pop();
        self.foundations[suit_index(card.suit)] = Some(card);
        Ok(())
    }

    fn column_found(&mut self, from: usize) -> Result<(), Illegal> {
        let placed = self.tableau.get(from).and_then(|column| column.last()).copied().ok_or(Illegal::Illegal)?;
        if !placed.face_up || !fits_foundation(placed.card, &self.foundations) {
            return Err(Illegal::Illegal);
        }
        self.tableau[from].pop();
        self.reveal(from);
        self.foundations[suit_index(placed.card.suit)] = Some(placed.card);
        Ok(())
    }

    fn transfer(&mut self, from: usize, count: usize, to: usize) -> Result<(), Illegal> {
        if from == to || from >= COLUMNS || to >= COLUMNS || self.liftable_from(from, self.tableau[from].len().saturating_sub(count)) != Some(count) {
            return Err(Illegal::Illegal);
        }
        let moving = self.tableau[from][self.tableau[from].len() - count].card;
        let under = self.tableau[to].last().map(|placed| placed.card);
        if !can_build(moving, under) {
            return Err(Illegal::Illegal);
        }
        let start = self.tableau[from].len() - count;
        let stack = self.tableau[from].split_off(start);
        self.tableau[to].extend(stack);
        self.reveal(from);
        Ok(())
    }

    fn reveal(&mut self, column: usize) {
        if let Some(top) = self.tableau[column].last_mut() {
            top.face_up = true;
        }
    }
}

fn suit_index(suit: Suit) -> usize {
    Suit::ALL.iter().position(|item| *item == suit).unwrap_or(0)
}

fn alternating(cards: &[Placed]) -> bool {
    cards.windows(2).all(|pair| {
        pair[0].card.suit.is_red() != pair[1].card.suit.is_red()
            && pair[0].card.rank.spider_value() == pair[1].card.rank.spider_value() + 1
    })
}

fn can_build(card: Card, under: Option<Card>) -> bool {
    match under {
        None => card.rank == Rank::King,
        Some(under) => {
            card.suit.is_red() != under.suit.is_red()
                && card.rank.spider_value() + 1 == under.rank.spider_value()
        }
    }
}

fn fits_foundation(card: Card, foundations: &[Option<Card>; 4]) -> bool {
    match foundations[suit_index(card.suit)] {
        None => card.rank == Rank::Ace,
        Some(top) => top.suit == card.suit && card.rank.spider_value() == top.rank.spider_value() + 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deal_uses_one_deck() {
        let game = Game::deal(7);
        let mut cards: Vec<_> = game.stock.iter().copied().chain(game.waste.iter().copied()).collect();
        for column in &game.tableau {
            cards.extend(column.iter().map(|placed| placed.card));
        }
        cards.sort_by_key(|card| (card.suit as u8, card.rank as u8));
        cards.dedup();
        assert_eq!(cards.len(), 52);
        assert_eq!(game.tableau[0].len(), 1);
        assert_eq!(game.tableau[6].len(), 7);
        assert!(game.tableau[6].last().unwrap().face_up);
        assert!(!game.tableau[6][0].face_up);
    }

    #[test]
    fn draw_three_then_recycle() {
        let mut game = Game::deal(3);
        let stock = game.stock.len();
        game.apply(Move::Draw).unwrap();
        assert_eq!(game.waste.len(), 3);
        assert_eq!(game.stock.len(), stock - 3);
        game.stock.clear();
        game.apply(Move::Recycle).unwrap();
        assert!(game.waste.is_empty());
        assert_eq!(game.stock.len(), 3);
    }
}
