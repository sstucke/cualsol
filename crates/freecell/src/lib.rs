use cualsol_engine::{shuffle, Card, Rank, Rng, Suit};

pub const CASCADES: usize = 8;
pub const FREECELLS: usize = 4;
pub const FOUNDATIONS: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Placed {
    pub card: Card,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Game {
    pub seed: u64,
    pub cascades: [Vec<Placed>; CASCADES],
    pub freecells: [Option<Card>; FREECELLS],
    pub foundations: [Option<Card>; FOUNDATIONS],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Move {
    CascadeToCascade { from: usize, to: usize },
    /// Secuencia alternada, de `count` cartas, de una columna a otra.
    Run { from: usize, count: usize, to: usize },
    CascadeToFreecell { from: usize, cell: usize },
    FreecellToCascade { cell: usize, to: usize },
    FreecellToFreecell { from: usize, to: usize },
    ToFoundation { from: Source },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Cascade(usize),
    Freecell(usize),
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum Illegal {
    #[error("origen vacío o fuera de rango")]
    Source,
    #[error("la celda libre está ocupada o no existe")]
    Freecell,
    #[error("no se puede apoyar esa carta")]
    Build,
    #[error("esa carta no entra en la fundación")]
    Foundation,
}

impl Game {
    /// Un mazo, columnas 0..3 con 7 cartas y 4..7 con 6. Todo boca arriba.
    pub fn deal(seed: u64) -> Self {
        let mut cards = Vec::with_capacity(52);
        for suit in Suit::ALL {
            for rank in Rank::ALL {
                cards.push(Card::new(suit, rank));
            }
        }
        let mut rng = Rng::from_seed(seed);
        shuffle(&mut cards, &mut rng);

        let mut cascades: [Vec<Placed>; CASCADES] = Default::default();
        for (index, card) in cards.into_iter().enumerate() {
            cascades[index % CASCADES].push(Placed { card });
        }
        Self {
            seed,
            cascades,
            freecells: [None; FREECELLS],
            foundations: [None; FOUNDATIONS],
        }
    }

    pub fn apply(&mut self, movement: Move) -> Result<(), Illegal> {
        match movement {
            Move::Run { from, count, to } => self.transfer_run(from, count, to),
            Move::FreecellToFreecell { from, to } => {
                if from == to || from >= FREECELLS || to >= FREECELLS {
                    return Err(Illegal::Freecell);
                }
                if self.freecells[to].is_some() {
                    return Err(Illegal::Freecell);
                }
                let Some(card) = self.freecells[from].take() else {
                    return Err(Illegal::Source);
                };
                self.freecells[to] = Some(card);
                Ok(())
            }
            Move::CascadeToCascade { from, to } => {
                let card = self.take_cascade(from)?;
                if !can_build(card, self.cascades[to].last().map(|placed| placed.card)) {
                    self.cascades[from].push(Placed { card });
                    return Err(Illegal::Build);
                }
                self.cascades[to].push(Placed { card });
                Ok(())
            }
            Move::CascadeToFreecell { from, cell } => {
                if cell >= FREECELLS || self.freecells[cell].is_some() {
                    return Err(Illegal::Freecell);
                }
                let card = self.take_cascade(from)?;
                self.freecells[cell] = Some(card);
                Ok(())
            }
            Move::FreecellToCascade { cell, to } => {
                let Some(card) = self.freecells.get(cell).copied().flatten() else {
                    return Err(Illegal::Source);
                };
                if to >= CASCADES || !can_build(card, self.cascades[to].last().map(|placed| placed.card))
                {
                    return Err(Illegal::Build);
                }
                self.freecells[cell] = None;
                self.cascades[to].push(Placed { card });
                Ok(())
            }
            Move::ToFoundation { from } => {
                let card = match from {
                    Source::Cascade(index) => self.take_cascade(index)?,
                    Source::Freecell(index) => {
                        let Some(card) = self.freecells.get_mut(index).and_then(|cell| cell.take())
                        else {
                            return Err(Illegal::Source);
                        };
                        card
                    }
                };
                if !can_found(card, &self.foundations) {
                    self.put_back(from, card);
                    return Err(Illegal::Foundation);
                }
                let slot = suit_index(card.suit);
                self.foundations[slot] = Some(card);
                Ok(())
            }
        }
    }

    pub fn won(&self) -> bool {
        self.foundations
            .iter()
            .all(|top| top.is_some_and(|card| card.rank == Rank::King))
    }

    /// Cuántas cartas se pueden levantar desde `row` (secuencia alternada que cabe en las celdas libres).
    pub fn liftable_from(&self, column: usize, row: usize) -> Option<usize> {
        let pile = self.cascades.get(column)?;
        let tail = pile.get(row..)?;
        if tail.is_empty() || !is_alternating_run(tail) {
            return None;
        }
        if tail.len() > self.max_movable(false) {
            return None;
        }
        Some(tail.len())
    }

    pub fn accepts_foundation(&self, card: Card) -> bool {
        can_found(card, &self.foundations)
    }

    /// Siguiente carta que puede ir a la fundación sin cerrar una bajada útil.
    /// Hace falta que los dos palos del otro color ya tengan al menos el rango anterior.
    pub fn next_safe_foundation(&self) -> Option<Move> {
        for (index, column) in self.cascades.iter().enumerate() {
            if let Some(top) = column.last() {
                if self.safe_to_foundation(top.card) {
                    return Some(Move::ToFoundation {
                        from: Source::Cascade(index),
                    });
                }
            }
        }
        for (index, cell) in self.freecells.iter().enumerate() {
            if let Some(card) = cell {
                if self.safe_to_foundation(*card) {
                    return Some(Move::ToFoundation {
                        from: Source::Freecell(index),
                    });
                }
            }
        }
        None
    }

    fn safe_to_foundation(&self, card: Card) -> bool {
        if !can_found(card, &self.foundations) {
            return false;
        }
        let need = card.rank.spider_value().saturating_sub(1);
        if need == 0 {
            return true;
        }
        Suit::ALL
            .iter()
            .filter(|suit| suit.is_red() != card.suit.is_red())
            .all(|suit| {
                self.foundations[suit_index(*suit)]
                    .is_some_and(|top| top.rank.spider_value() >= need)
            })
    }

    fn max_movable(&self, into_empty: bool) -> usize {
        let free = self.freecells.iter().filter(|cell| cell.is_none()).count();
        let mut empty = self.cascades.iter().filter(|column| column.is_empty()).count();
        if into_empty {
            empty = empty.saturating_sub(1);
        }
        (free + 1) << empty.min(8)
    }

    fn transfer_run(&mut self, from: usize, count: usize, to: usize) -> Result<(), Illegal> {
        if from >= CASCADES || to >= CASCADES || from == to || count == 0 {
            return Err(Illegal::Source);
        }
        let source_len = self.cascades[from].len();
        if count > source_len {
            return Err(Illegal::Source);
        }
        let start = source_len - count;
        if !is_alternating_run(&self.cascades[from][start..]) {
            return Err(Illegal::Build);
        }
        let into_empty = self.cascades[to].is_empty();
        if count > self.max_movable(into_empty) {
            return Err(Illegal::Build);
        }
        let moving = self.cascades[from][start].card;
        if !can_build(moving, self.cascades[to].last().map(|placed| placed.card)) {
            return Err(Illegal::Build);
        }
        let stack = self.cascades[from].split_off(start);
        self.cascades[to].extend(stack);
        Ok(())
    }

    fn take_cascade(&mut self, index: usize) -> Result<Card, Illegal> {
        self.cascades
            .get_mut(index)
            .and_then(|column| column.pop())
            .map(|placed| placed.card)
            .ok_or(Illegal::Source)
    }

    fn put_back(&mut self, source: Source, card: Card) {
        match source {
            Source::Cascade(index) => self.cascades[index].push(Placed { card }),
            Source::Freecell(index) => self.freecells[index] = Some(card),
        }
    }
}

fn suit_index(suit: Suit) -> usize {
    Suit::ALL.iter().position(|item| *item == suit).unwrap_or(0)
}

fn is_alternating_run(cards: &[Placed]) -> bool {
    cards.windows(2).all(|pair| {
        pair[0].card.suit.is_red() != pair[1].card.suit.is_red()
            && pair[0].card.rank.spider_value() == pair[1].card.rank.spider_value() + 1
    })
}

fn can_build(card: Card, under: Option<Card>) -> bool {
    match under {
        None => true,
        Some(under) => {
            card.suit.is_red() != under.suit.is_red()
                && card.rank.spider_value() + 1 == under.rank.spider_value()
        }
    }
}

fn can_found(card: Card, foundations: &[Option<Card>; FOUNDATIONS]) -> bool {
    let top = foundations[suit_index(card.suit)];
    match (card.rank, top) {
        (Rank::Ace, None) => true,
        (_, Some(top)) => {
            top.suit == card.suit && card.rank.spider_value() == top.rank.spider_value() + 1
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deal_uses_one_deck() {
        let game = Game::deal(3);
        let count: usize = game.cascades.iter().map(|column| column.len()).sum();
        assert_eq!(count, 52);
        assert_eq!(game.cascades[0].len(), 7);
        assert_eq!(game.cascades[4].len(), 6);
        assert!(game.freecells.iter().all(|cell| cell.is_none()));
    }

    #[test]
    fn freecell_holds_one_card() {
        let mut game = Game::deal(3);
        let card = game.cascades[0].last().unwrap().card;
        assert!(game
            .apply(Move::CascadeToFreecell { from: 0, cell: 0 })
            .is_ok());
        assert_eq!(game.freecells[0], Some(card));
        assert_eq!(
            game.apply(Move::CascadeToFreecell { from: 1, cell: 0 }),
            Err(Illegal::Freecell)
        );
    }

    #[test]
    fn queen_of_clubs_builds_on_king_of_diamonds() {
        let mut game = Game::deal(1);
        game.cascades[0].clear();
        game.cascades[1].clear();
        game.cascades[0].push(Placed {
            card: Card::new(Suit::Clubs, Rank::Queen),
        });
        game.cascades[1].push(Placed {
            card: Card::new(Suit::Diamonds, Rank::King),
        });
        assert!(game
            .apply(Move::Run {
                from: 0,
                count: 1,
                to: 1,
            })
            .is_ok());
        assert_eq!(
            game.cascades[1].last().map(|placed| placed.card),
            Some(Card::new(Suit::Clubs, Rank::Queen))
        );
    }

    #[test]
    fn alternating_run_moves_onto_a_higher_card() {
        let mut game = Game::deal(1);
        game.cascades[0].clear();
        game.cascades[1].clear();
        game.cascades[0].push(Placed {
            card: Card::new(Suit::Spades, Rank::Five),
        });
        game.cascades[0].push(Placed {
            card: Card::new(Suit::Hearts, Rank::Four),
        });
        game.cascades[1].push(Placed {
            card: Card::new(Suit::Hearts, Rank::Six),
        });
        assert_eq!(game.liftable_from(0, 0), Some(2));
        assert!(game.apply(Move::Run { from: 0, count: 2, to: 1 }).is_ok());
        assert!(game.cascades[0].is_empty());
        assert_eq!(game.cascades[1].len(), 3);
    }

    #[test]
    fn two_waits_for_both_opposite_aces() {
        let mut game = Game::deal(1);
        for column in &mut game.cascades {
            column.clear();
        }
        game.cascades[0].push(Placed {
            card: Card::new(Suit::Hearts, Rank::Two),
        });
        assert!(game.next_safe_foundation().is_none());
        game.foundations[suit_index(Suit::Spades)] = Some(Card::new(Suit::Spades, Rank::Ace));
        assert!(game.next_safe_foundation().is_none());
        game.foundations[suit_index(Suit::Clubs)] = Some(Card::new(Suit::Clubs, Rank::Ace));
        game.foundations[suit_index(Suit::Hearts)] = Some(Card::new(Suit::Hearts, Rank::Ace));
        assert!(matches!(
            game.next_safe_foundation(),
            Some(Move::ToFoundation { from: Source::Cascade(0) })
        ));
    }
}
