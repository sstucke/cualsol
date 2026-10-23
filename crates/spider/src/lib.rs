use cualsol_engine::{shuffle, Card, Rank, Rng, Suit};

pub const COLUMNS: usize = 10;
pub const DECKS: usize = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Variant {
    OneSuit,
    TwoSuits,
    FourSuits,
}

impl Variant {
    pub fn label(self) -> &'static str {
        match self {
            Variant::OneSuit => "Spider 1 palo",
            Variant::TwoSuits => "Spider 2 palos",
            Variant::FourSuits => "Spider 4 palos",
        }
    }

    fn suits(self) -> &'static [Suit] {
        match self {
            Variant::OneSuit => &[Suit::Spades],
            Variant::TwoSuits => &[Suit::Spades, Suit::Hearts],
            Variant::FourSuits => &Suit::ALL,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Placed {
    pub card: Card,
    pub face_up: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Game {
    pub variant: Variant,
    pub seed: u64,
    pub tableau: [Vec<Placed>; COLUMNS],
    pub stock: Vec<Card>,
    pub completed: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Move {
    /// Mueve `count` cartas boca arriba del final de `from` a `to`.
    Transfer { from: usize, count: usize, to: usize },
    /// Reparte una carta boca arriba en cada columna.
    DealStock,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum Illegal {
    #[error("columna fuera de rango")]
    Column,
    #[error("no hay tantas cartas boca arriba para mover")]
    Count,
    #[error("la secuencia no es del mismo palo y descendente")]
    Sequence,
    #[error("la carta no apoya sobre el destino")]
    Build,
    #[error("el mazo no alcanza para repartir")]
    Stock,
    #[error("hay una columna vacía")]
    EmptyColumn,
}

impl Game {
    pub fn deal(variant: Variant, seed: u64) -> Self {
        let mut cards = Vec::with_capacity(104);
        let suits = variant.suits();
        let copies = (DECKS * Suit::ALL.len()) / suits.len();
        for _ in 0..copies {
            for &suit in suits {
                for rank in Rank::ALL {
                    cards.push(Card::new(suit, rank));
                }
            }
        }
        let mut rng = Rng::from_seed(seed);
        shuffle(&mut cards, &mut rng);

        let mut tableau: [Vec<Placed>; COLUMNS] = Default::default();
        let mut index = 0;
        for column in 0..COLUMNS {
            let height = if column < 4 { 6 } else { 5 };
            for row in 0..height {
                let face_up = row + 1 == height;
                tableau[column].push(Placed {
                    card: cards[index],
                    face_up,
                });
                index += 1;
            }
        }

        Self {
            variant,
            seed,
            tableau,
            stock: cards[index..].to_vec(),
            completed: 0,
        }
    }

    pub fn apply(&mut self, movement: Move) -> Result<(), Illegal> {
        match movement {
            Move::DealStock => self.deal_stock(),
            Move::Transfer { from, count, to } => self.transfer(from, count, to),
        }
    }

    fn deal_stock(&mut self) -> Result<(), Illegal> {
        if self.stock.len() < COLUMNS {
            return Err(Illegal::Stock);
        }
        if self.tableau.iter().any(|column| column.is_empty()) {
            return Err(Illegal::EmptyColumn);
        }
        let dealt: Vec<Card> = self.stock.drain(..COLUMNS).collect();
        for (column, card) in self.tableau.iter_mut().zip(dealt) {
            column.push(Placed {
                card,
                face_up: true,
            });
        }
        self.remove_completed();
        Ok(())
    }

    fn transfer(&mut self, from: usize, count: usize, to: usize) -> Result<(), Illegal> {
        if from >= COLUMNS || to >= COLUMNS || from == to || count == 0 {
            return Err(Illegal::Column);
        }
        let source_len = self.tableau[from].len();
        if count > source_len {
            return Err(Illegal::Count);
        }
        let start = source_len - count;
        if self.tableau[from][start..]
            .iter()
            .any(|placed| !placed.face_up)
        {
            return Err(Illegal::Count);
        }
        if !is_suit_run(&self.tableau[from][start..]) {
            return Err(Illegal::Sequence);
        }
        if let Some(destination) = self.tableau[to].last() {
            let moving = self.tableau[from][start].card;
            if !builds_on(moving, destination.card) {
                return Err(Illegal::Build);
            }
        }
        let moving = self.tableau[from].split_off(start);
        self.tableau[to].extend(moving);
        if let Some(revealed) = self.tableau[from].last_mut() {
            revealed.face_up = true;
        }
        self.remove_completed();
        Ok(())
    }

    fn remove_completed(&mut self) {
        for column in &mut self.tableau {
            if suit_run_from_king(column) {
                let len = column.len();
                column.truncate(len - 13);
                if let Some(revealed) = column.last_mut() {
                    revealed.face_up = true;
                }
                self.completed = self.completed.saturating_add(1);
            }
        }
    }

    pub fn won(&self) -> bool {
        self.completed == 8
    }

    /// Cartas boca arriba, del mismo palo y descendentes, desde `row` hasta el final.
    pub fn liftable_from(&self, column: usize, row: usize) -> Option<usize> {
        let pile = self.tableau.get(column)?;
        let tail = pile.get(row..)?;
        if tail.is_empty() || tail.iter().any(|placed| !placed.face_up) || !is_suit_run(tail) {
            return None;
        }
        Some(tail.len())
    }

    /// Mueve una bajada del mismo palo sobre la carta del mismo palo que la continúa.
    pub fn next_same_suit_build(&self) -> Option<Move> {
        for from in 0..COLUMNS {
            let pile = &self.tableau[from];
            for row in 0..pile.len() {
                let Some(count) = self.liftable_from(from, row) else {
                    continue;
                };
                let moving = pile[row].card;
                for to in 0..COLUMNS {
                    if to == from {
                        continue;
                    }
                    let Some(dest) = self.tableau[to].last() else {
                        continue;
                    };
                    if dest.face_up && dest.card.suit == moving.suit && builds_on(moving, dest.card)
                    {
                        return Some(Move::Transfer { from, count, to });
                    }
                }
            }
        }
        None
    }
}

fn is_suit_run(cards: &[Placed]) -> bool {
    cards.windows(2).all(|pair| {
        pair[0].card.suit == pair[1].card.suit
            && pair[0].card.rank.spider_value() == pair[1].card.rank.spider_value() + 1
    })
}

fn builds_on(moving: Card, under: Card) -> bool {
    moving.rank.spider_value() + 1 == under.rank.spider_value()
}

fn suit_run_from_king(column: &[Placed]) -> bool {
    if column.len() < 13 {
        return false;
    }
    let tail = &column[column.len() - 13..];
    tail.iter().all(|placed| placed.face_up)
        && tail[0].card.rank == Rank::King
        && is_suit_run(tail)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deal_counts_match_spider() {
        for variant in [Variant::OneSuit, Variant::TwoSuits, Variant::FourSuits] {
            let game = Game::deal(variant, 7);
            let tableau: usize = game.tableau.iter().map(|column| column.len()).sum();
            assert_eq!(tableau, 54);
            assert_eq!(game.stock.len(), 50);
            assert!(game.tableau.iter().all(|column| column.last().unwrap().face_up));
            assert_eq!(game.tableau[0].len(), 6);
            assert_eq!(game.tableau[9].len(), 5);
            let suits = game
                .tableau
                .iter()
                .flatten()
                .map(|placed| placed.card.suit)
                .chain(game.stock.iter().map(|card| card.suit))
                .collect::<std::collections::BTreeSet<_>>();
            assert_eq!(suits.len(), variant.suits().len());
        }
    }

    #[test]
    fn same_seed_deals_the_same_board() {
        assert_eq!(
            Game::deal(Variant::FourSuits, 42),
            Game::deal(Variant::FourSuits, 42)
        );
    }

    #[test]
    fn stock_deals_one_row_and_stops_when_short() {
        let mut game = Game::deal(Variant::OneSuit, 1);
        assert!(game.apply(Move::DealStock).is_ok());
        assert_eq!(game.stock.len(), 40);
        assert!(game.tableau.iter().all(|column| column.last().unwrap().face_up));
        game.stock.truncate(3);
        assert_eq!(game.apply(Move::DealStock), Err(Illegal::Stock));
    }

    #[test]
    fn same_suit_run_builds_automatically() {
        let mut game = Game::deal(Variant::OneSuit, 1);
        for column in &mut game.tableau {
            column.clear();
        }
        game.tableau[0].push(Placed {
            card: Card::new(Suit::Spades, Rank::Five),
            face_up: true,
        });
        game.tableau[0].push(Placed {
            card: Card::new(Suit::Spades, Rank::Four),
            face_up: true,
        });
        game.tableau[1].push(Placed {
            card: Card::new(Suit::Spades, Rank::Six),
            face_up: true,
        });
        assert_eq!(
            game.next_same_suit_build(),
            Some(Move::Transfer {
                from: 0,
                count: 2,
                to: 1
            })
        );
    }
}
