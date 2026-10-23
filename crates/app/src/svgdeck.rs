use std::path::Path;

use resvg::tiny_skia::{Pixmap, Transform};
use resvg::usvg::{self, Tree};

use cualsol_engine::Card;

pub struct SvgDeck {
    tree: Tree,
}

impl SvgDeck {
    pub fn load(path: &Path) -> Option<Self> {
        let data = std::fs::read(path).ok()?;
        let tree = Tree::from_data(&data, &usvg::Options::default()).ok()?;
        let deck = Self { tree };
        if deck.has("1_spade") && deck.has(BACK_ELEMENT) {
            return Some(deck);
        }
        let exposed = expose_hidden_cards(&data);
        let tree = Tree::from_data(&exposed, &usvg::Options::default()).ok()?;
        Some(Self { tree })
    }

    pub fn has(&self, id: &str) -> bool {
        self.tree.node_by_id(id).is_some()
            || self.tree.node_by_id(&format!("cual-{id}")).is_some()
    }

    pub fn render(&self, id: &str, width: u32) -> Option<Vec<u8>> {
        let node = self
            .tree
            .node_by_id(id)
            .or_else(|| self.tree.node_by_id(&format!("cual-{id}")))?;
        let bbox = node.abs_layer_bounding_box()?;
        if bbox.width() < 1.0 || bbox.height() < 1.0 {
            return None;
        }
        let width = width.clamp(8, 400);
        let scale = width as f32 / bbox.width();
        let height = (bbox.height() * scale).ceil().clamp(8.0, 600.0) as u32;
        let mut pixmap = Pixmap::new(width, height)?;
        resvg::render_node(node, Transform::from_scale(scale, scale), &mut pixmap.as_mut())?;
        Some(pixmap.take())
    }
}

pub fn card_element(card: Card) -> String {
    let rank = match card.rank {
        cualsol_engine::Rank::Ace => "1",
        cualsol_engine::Rank::Two => "2",
        cualsol_engine::Rank::Three => "3",
        cualsol_engine::Rank::Four => "4",
        cualsol_engine::Rank::Five => "5",
        cualsol_engine::Rank::Six => "6",
        cualsol_engine::Rank::Seven => "7",
        cualsol_engine::Rank::Eight => "8",
        cualsol_engine::Rank::Nine => "9",
        cualsol_engine::Rank::Ten => "10",
        cualsol_engine::Rank::Jack => "jack",
        cualsol_engine::Rank::Queen => "queen",
        cualsol_engine::Rank::King => "king",
    };
    let suit = match card.suit {
        cualsol_engine::Suit::Clubs => "club",
        cualsol_engine::Suit::Diamonds => "diamond",
        cualsol_engine::Suit::Hearts => "heart",
        cualsol_engine::Suit::Spades => "spade",
    };
    format!("{rank}_{suit}")
}

pub const BACK_ELEMENT: &str = "back";

pub fn write_raster(deck: &SvgDeck, dir: &Path, width: u32) {
    std::fs::create_dir_all(dir).expect("carpeta");
    for suit in cualsol_engine::Suit::ALL {
        for rank in cualsol_engine::Rank::ALL {
            let card = cualsol_engine::Card::new(suit, rank);
            let id = card_element(card);
            let pixels = deck
                .render(&id, width)
                .unwrap_or_else(|| panic!("sin {id}"));
            save_straight(dir.join(card.face_file_name()), width, &pixels);
        }
    }
    let back = deck.render(BACK_ELEMENT, width).expect("dorso");
    save_straight(dir.join("back.png"), width, &back);
}

fn save_straight(path: std::path::PathBuf, width: u32, pixels: &[u8]) {
    let height = pixels.len() / (width as usize * 4);
    let mut straight = pixels.to_vec();
    for pixel in straight.chunks_exact_mut(4) {
        let alpha = pixel[3] as u32;
        if alpha == 0 {
            pixel[0] = 0;
            pixel[1] = 0;
            pixel[2] = 0;
            continue;
        }
        pixel[0] = ((pixel[0] as u32 * 255) / alpha).min(255) as u8;
        pixel[1] = ((pixel[1] as u32 * 255) / alpha).min(255) as u8;
        pixel[2] = ((pixel[2] as u32 * 255) / alpha).min(255) as u8;
    }
    let image = image::RgbaImage::from_raw(width, height as u32, straight).expect("tamaño");
    image.save(path).expect("png");
}

fn expose_hidden_cards(data: &[u8]) -> Vec<u8> {
    let text = String::from_utf8_lossy(data);
    let Some(end) = text.rfind("</svg>") else {
        return data.to_vec();
    };
    let mut uses = String::from("<g id=\"cual-expose\">\n");
    for rank in [
        "1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "jack", "queen", "king",
    ] {
        for suit in ["club", "diamond", "heart", "spade"] {
            let id = format!("{rank}_{suit}");
            uses.push_str(&format!(
                "<use id=\"cual-{id}\" xlink:href=\"#{id}\" href=\"#{id}\"/>\n"
            ));
        }
    }
    uses.push_str("<use id=\"cual-back\" xlink:href=\"#back\" href=\"#back\"/>\n</g>\n");
    let mut out = String::new();
    out.push_str(&text[..end]);
    out.push_str(&uses);
    out.push_str(&text[end..]);
    out.into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;
    use cualsol_engine::{Rank, Suit};

    #[test]
    fn standard_ace_renders() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets/carddecks/standard/standard.svg");
        let deck = SvgDeck::load(&path).expect("mazo");
        let id = card_element(Card::new(Suit::Spades, Rank::Ace));
        let pixels = deck.render(&id, 80).expect("carta");
        assert!(pixels.len() > 1000);
        assert!(deck.has(BACK_ELEMENT));
    }

    #[test]
    #[ignore]
    fn export_rasters() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/carddecks");
        let decks = [
            ("standard/standard.svg", "standard/raster"),
            ("xskat-french/french.svg", "xskat-french/raster"),
            ("xskat-german/german.svg", "xskat-german/raster"),
            ("penguins/penguins.svg", "penguins/raster"),
            ("tigullio/tigullio-international.svg", "tigullio/raster"),
            ("nicu-ornamental/ornamental.svg", "nicu-ornamental/raster"),
            ("nicu-white/white.svg", "nicu-white/raster"),
        ];
        for (svg, raster) in decks {
            let deck = SvgDeck::load(&root.join(svg)).unwrap_or_else(|| panic!("{svg}"));
            write_raster(&deck, &root.join(raster), 220);
        }
    }

    #[test]
    fn licensed_decks_have_ace_and_back() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/carddecks");
        let files = [
            "standard/standard.svg",
            "xskat-french/french.svg",
            "xskat-german/german.svg",
            "penguins/penguins.svg",
            "tigullio/tigullio-international.svg",
            "nicu-ornamental/ornamental.svg",
            "nicu-white/white.svg",
        ];
        let ace = card_element(Card::new(Suit::Spades, Rank::Ace));
        for file in files {
            let deck = SvgDeck::load(&root.join(file)).unwrap_or_else(|| panic!("no carga {file}"));
            assert!(deck.has(&ace), "{file} sin {ace}");
            assert!(deck.has(BACK_ELEMENT), "{file} sin dorso");
            assert!(deck.render(&ace, 40).is_some(), "{file} no dibuja");
        }
    }
}
