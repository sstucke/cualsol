use cualsol_engine::{shuffle, Card, Rank, Rng, Suit};

pub const PLAYERS: usize = 4;
pub const HUMAN: usize = 0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Pass {
    Left,
    Right,
    Across,
    Hold,
}

impl Pass {
    pub fn label(self) -> &'static str {
        match self {
            Pass::Left => "a la izquierda",
            Pass::Right => "a la derecha",
            Pass::Across => "al frente",
            Pass::Hold => "sin pasar",
        }
    }

    fn from_hand(hand: u32) -> Self {
        match hand % 4 {
            0 => Pass::Left,
            1 => Pass::Right,
            2 => Pass::Across,
            _ => Pass::Hold,
        }
    }

    fn target(self, seat: usize) -> usize {
        let step = match self {
            Pass::Left => 1,
            Pass::Right => 3,
            Pass::Across => 2,
            Pass::Hold => 0,
        };
        (seat + step) % PLAYERS
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Game {
    pub seed: u64,
    pub hand_no: u32,
    pub scores: [i32; PLAYERS],
    pub hands: [Vec<Card>; PLAYERS],
    pub pass: Pass,
    pub chosen: Vec<Card>,
    pub passing: bool,
    pub trick: Vec<(usize, Card)>,
    pub leader: usize,
    pub turn: usize,
    pub broken: bool,
    pub taken: [Vec<Card>; PLAYERS],
    pub finished: bool,
    pub last_note: String,
}

impl Game {
    pub fn deal(seed: u64) -> Self {
        let mut game = Self {
            seed,
            hand_no: 0,
            scores: [0; PLAYERS],
            hands: Default::default(),
            pass: Pass::Left,
            chosen: Vec::new(),
            passing: true,
            trick: Vec::new(),
            leader: 0,
            turn: 0,
            broken: false,
            taken: Default::default(),
            finished: false,
            last_note: String::new(),
        };
        game.redeal_hand();
        game
    }

    pub fn toggle_pass(&mut self, card: Card) {
        if !self.passing || self.pass == Pass::Hold || self.finished {
            return;
        }
        if let Some(index) = self.chosen.iter().position(|item| *item == card) {
            self.chosen.remove(index);
            return;
        }
        if self.chosen.len() < 3 && self.hands[HUMAN].contains(&card) {
            self.chosen.push(card);
        }
    }

    pub fn confirm_pass(&mut self) -> bool {
        if !self.passing || self.finished {
            return false;
        }
        if self.pass == Pass::Hold {
            self.passing = false;
            self.prepare_lead();
            return true;
        }
        if self.chosen.len() != 3 {
            return false;
        }
        let mut gifts = [Vec::new(), Vec::new(), Vec::new(), Vec::new()];
        gifts[HUMAN] = self.chosen.clone();
        for seat in 1..PLAYERS {
            gifts[seat] = ai_pass(&self.hands[seat]);
        }
        for seat in 0..PLAYERS {
            let give = &gifts[seat];
            self.hands[seat].retain(|card| !give.contains(card));
            let into = self.pass.target(seat);
            self.hands[into].extend(give.iter().copied());
        }
        for hand in &mut self.hands {
            sort_hand(hand);
        }
        self.chosen.clear();
        self.passing = false;
        self.prepare_lead();
        true
    }

    pub fn play_human(&mut self, card: Card) -> bool {
        if self.finished || self.passing || self.turn != HUMAN || self.trick.len() == 4 {
            return false;
        }
        if !self.legal(HUMAN).contains(&card) {
            return false;
        }
        self.play(HUMAN, card);
        true
    }

    /// Juega la carta de la computadora si es su turno y la baza no está llena.
    pub fn play_ai(&mut self) -> bool {
        if self.finished || self.passing || self.turn == HUMAN || self.trick.len() == 4 {
            return false;
        }
        let seat = self.turn;
        let card = ai_play(self, seat);
        self.play(seat, card);
        true
    }

    pub fn trick_full(&self) -> bool {
        self.trick.len() == 4
    }

    pub fn close_trick(&mut self) {
        if !self.trick_full() {
            return;
        }
        let winner = trick_winner(&self.trick);
        let cards: Vec<Card> = self.trick.iter().map(|(_, card)| *card).collect();
        let points: i32 = cards.iter().map(|card| penalty(*card)).sum();
        self.taken[winner].extend(cards);
        if self.taken.iter().flatten().count() == 52 {
            self.score_hand();
            self.trick.clear();
        } else {
            self.trick.clear();
            self.leader = winner;
            self.turn = winner;
            self.last_note = format!("{} se llevó la baza.", seat_name(winner));
        }
        let _ = points;
    }

    pub fn trick_taker(&self) -> Option<usize> {
        if self.trick.len() < 4 {
            None
        } else {
            Some(trick_winner(&self.trick))
        }
    }

    pub fn legal(&self, seat: usize) -> Vec<Card> {
        let hand = &self.hands[seat];
        if self.trick.is_empty() {
            let lead: Vec<Card> = hand
                .iter()
                .copied()
                .filter(|card| self.broken || card.suit != Suit::Hearts || hand.iter().all(|item| item.suit == Suit::Hearts))
                .filter(|card| !first_trick_blocked(self, *card))
                .collect();
            if lead.is_empty() { hand.clone() } else { lead }
        } else {
            let led = self.trick[0].1.suit;
            let follow: Vec<Card> = hand.iter().copied().filter(|card| card.suit == led).collect();
            let pool = if follow.is_empty() { hand.clone() } else { follow };
            let blocked: Vec<Card> = pool.into_iter().filter(|card| !first_trick_blocked(self, *card) || hand.iter().all(|item| first_trick_blocked(self, *item))).collect();
            if blocked.is_empty() { hand.clone() } else { blocked }
        }
    }

    fn play(&mut self, seat: usize, card: Card) {
        if let Some(index) = self.hands[seat].iter().position(|item| *item == card) {
            self.hands[seat].remove(index);
        }
        if card.suit == Suit::Hearts {
            self.broken = true;
        }
        self.trick.push((seat, card));
        if self.trick.len() < 4 {
            self.turn = (seat + 1) % PLAYERS;
        }
    }

    fn prepare_lead(&mut self) {
        self.leader = self
            .hands
            .iter()
            .position(|hand| hand.contains(&Card::new(Suit::Clubs, Rank::Two)))
            .unwrap_or(0);
        self.turn = self.leader;
        self.broken = false;
        self.trick.clear();
        self.last_note = format!("Sale {}.", seat_name(self.leader));
    }

    fn redeal_hand(&mut self) {
        let mut cards = Vec::with_capacity(52);
        for suit in Suit::ALL {
            for rank in Rank::ALL {
                cards.push(Card::new(suit, rank));
            }
        }
        let mut rng = Rng::from_seed(self.seed.wrapping_add(self.hand_no as u64 * 997));
        shuffle(&mut cards, &mut rng);
        for seat in 0..PLAYERS {
            self.hands[seat] = cards[seat * 13..(seat + 1) * 13].to_vec();
            sort_hand(&mut self.hands[seat]);
            self.taken[seat].clear();
        }
        self.pass = Pass::from_hand(self.hand_no);
        self.chosen.clear();
        self.passing = self.pass != Pass::Hold;
        self.trick.clear();
        self.broken = false;
        if self.pass == Pass::Hold {
            self.prepare_lead();
        } else {
            self.last_note = format!("Elegí 3 cartas para pasar {}.", self.pass.label());
        }
    }

    fn score_hand(&mut self) {
        let mut round = [0; PLAYERS];
        for seat in 0..PLAYERS {
            round[seat] = self.taken[seat].iter().map(|card| penalty(*card)).sum();
        }
        if let Some(moon) = round.iter().position(|score| *score == 26) {
            for seat in 0..PLAYERS {
                if seat != moon {
                    self.scores[seat] += 26;
                }
            }
            self.last_note = format!("{} se llevó la luna.", seat_name(moon));
        } else {
            for seat in 0..PLAYERS {
                self.scores[seat] += round[seat];
            }
            self.last_note = format!("Mano lista. Vos {}", round[HUMAN]);
        }
        if self.scores.iter().any(|score| *score >= 100) {
            self.finished = true;
            self.passing = false;
            self.turn = PLAYERS;
        } else {
            self.hand_no += 1;
            self.redeal_hand();
        }
    }
}

pub fn seat_name(seat: usize) -> &'static str {
    match seat {
        HUMAN => "Vos",
        1 => "Oeste",
        2 => "Norte",
        _ => "Este",
    }
}

pub fn penalty(card: Card) -> i32 {
    if card.suit == Suit::Hearts {
        1
    } else if card == Card::new(Suit::Spades, Rank::Queen) {
        13
    } else {
        0
    }
}

fn first_trick_blocked(game: &Game, card: Card) -> bool {
    game.taken.iter().all(|pile| pile.is_empty()) && game.trick.is_empty() && penalty(card) > 0 && game.hands.iter().flatten().any(|item| penalty(*item) == 0)
        || (game.taken.iter().all(|pile| pile.is_empty()) && penalty(card) > 0 && game.trick.iter().all(|(_, played)| penalty(*played) == 0) && game.hands[game.turn].iter().any(|item| penalty(*item) == 0 && (game.trick.is_empty() || item.suit == game.trick[0].1.suit || game.hands[game.turn].iter().all(|other| other.suit != game.trick[0].1.suit))))
}

fn trick_winner(trick: &[(usize, Card)]) -> usize {
    let lead = trick[0].1.suit;
    trick
        .iter()
        .filter(|(_, card)| card.suit == lead)
        .max_by_key(|(_, card)| card.rank.hearts_value())
        .map(|(seat, _)| *seat)
        .unwrap_or(trick[0].0)
}

fn sort_hand(hand: &mut [Card]) {
    hand.sort_by_key(|card| (suit_order(card.suit), card.rank.hearts_value()));
}

fn suit_order(suit: Suit) -> u8 {
    match suit {
        Suit::Clubs => 0,
        Suit::Diamonds => 1,
        Suit::Spades => 2,
        Suit::Hearts => 3,
    }
}

fn ai_pass(hand: &[Card]) -> Vec<Card> {
    let mut ranked = hand.to_vec();
    ranked.sort_by_key(|card| std::cmp::Reverse(danger(*card)));
    ranked.truncate(3);
    ranked
}

fn danger(card: Card) -> i32 {
    if card == Card::new(Suit::Spades, Rank::Queen) {
        100
    } else if card.suit == Suit::Hearts {
        40 + card.rank.hearts_value() as i32
    } else if card.suit == Suit::Spades && card.rank.hearts_value() >= 13 {
        30
    } else {
        card.rank.hearts_value() as i32
    }
}

fn ai_play(game: &Game, seat: usize) -> Card {
    let legal = game.legal(seat);
    let moon = shooting(game, seat);
    if game.trick.is_empty() {
        return lead_card(&legal, moon);
    }
    let lead = game.trick[0].1.suit;
    let winner = winning_card(game);
    let points = game.trick.iter().any(|(_, card)| penalty(*card) > 0);
    let same: Vec<Card> = legal.iter().copied().filter(|card| card.suit == lead).collect();
    if same.is_empty() {
        return dump(&legal, points || moon);
    }
    if moon {
        return highest(&same);
    }
    let under: Vec<Card> = same
        .iter()
        .copied()
        .filter(|card| card.rank.hearts_value() < winner.rank.hearts_value())
        .collect();
    if points {
        if under.is_empty() { lowest(&same) } else { highest(&under) }
    } else if game.trick.len() == 3 && under.is_empty() {
        highest(&same)
    } else {
        lowest(&same)
    }
}

fn shooting(game: &Game, seat: usize) -> bool {
    let hearts = game.hands[seat].iter().filter(|card| card.suit == Suit::Hearts).count();
    let queen = game.hands[seat].contains(&Card::new(Suit::Spades, Rank::Queen));
    let taken = game.taken[seat].iter().map(|card| penalty(*card)).sum::<i32>();
    taken >= 14 || (queen && hearts >= 6)
}

fn lead_card(legal: &[Card], moon: bool) -> Card {
    if moon {
        return highest(legal);
    }
    let safe: Vec<Card> = legal.iter().copied().filter(|card| card.suit != Suit::Hearts && *card != Card::new(Suit::Spades, Rank::Queen)).collect();
    let pool = if safe.is_empty() { legal } else { &safe };
    lowest(pool)
}

fn dump(legal: &[Card], dangerous: bool) -> Card {
    let queen = Card::new(Suit::Spades, Rank::Queen);
    if dangerous {
        if legal.contains(&queen) {
            return queen;
        }
        let hearts: Vec<Card> = legal.iter().copied().filter(|card| card.suit == Suit::Hearts).collect();
        if !hearts.is_empty() {
            return highest(&hearts);
        }
        return highest(legal);
    }
    lowest(legal)
}

fn winning_card(game: &Game) -> Card {
    let lead = game.trick[0].1.suit;
    game.trick
        .iter()
        .filter(|(_, card)| card.suit == lead)
        .map(|(_, card)| *card)
        .max_by_key(|card| card.rank.hearts_value())
        .unwrap_or(game.trick[0].1)
}

fn lowest(cards: &[Card]) -> Card {
    cards.iter().copied().min_by_key(|card| card.rank.hearts_value()).unwrap_or(cards[0])
}

fn highest(cards: &[Card]) -> Card {
    cards.iter().copied().max_by_key(|card| card.rank.hearts_value()).unwrap_or(cards[0])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deal_gives_thirteen_each() {
        let game = Game::deal(4);
        assert!(game.hands.iter().all(|hand| hand.len() == 13));
        assert!(game.passing);
    }

    #[test]
    fn must_follow_suit() {
        let mut game = Game::deal(1);
        game.passing = false;
        game.hands[0] = vec![Card::new(Suit::Clubs, Rank::Two), Card::new(Suit::Hearts, Rank::Ace)];
        game.trick = vec![(1, Card::new(Suit::Clubs, Rank::Five))];
        game.turn = 0;
        let legal = game.legal(0);
        assert_eq!(legal, vec![Card::new(Suit::Clubs, Rank::Two)]);
    }

    #[test]
    fn ace_beats_king() {
        let mut game = Game::deal(1);
        game.passing = false;
        game.trick = vec![
            (1, Card::new(Suit::Hearts, Rank::King)),
            (2, Card::new(Suit::Hearts, Rank::Ace)),
            (3, Card::new(Suit::Clubs, Rank::Ace)),
            (0, Card::new(Suit::Hearts, Rank::Two)),
        ];
        assert_eq!(trick_winner(&game.trick), 2);
    }

    #[test]
    fn queen_is_thirteen() {
        assert_eq!(penalty(Card::new(Suit::Spades, Rank::Queen)), 13);
        assert_eq!(penalty(Card::new(Suit::Hearts, Rank::Five)), 1);
    }
}
