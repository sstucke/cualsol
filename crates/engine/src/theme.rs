use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::Card;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackPattern {
    Solid,
    Diamond,
    Lines,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FaceStyle {
    /// Índices y palos dibujados por la aplicación.
    Classic,
    /// Rango grande al centro.
    Grande,
    /// Un color distinto por palo.
    FourColor,
    /// Carta oscura con tinta clara.
    Night,
}

/// Superficie del escritorio. La imagen, si carga, tapa el color.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tablecloth {
    pub color: Rgb,
    pub image: Option<PathBuf>,
}

/// Reverso. La imagen, si carga, tapa color y patrón.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CardBack {
    pub color: Rgb,
    pub pattern: BackPattern,
    pub image: Option<PathBuf>,
}

/// Caras. Cada PNG opcional sustituye el dibujo vectorial de esa carta.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CardFaces {
    pub style: FaceStyle,
    pub directory: Option<PathBuf>,
}

impl CardFaces {
    pub fn image_for(&self, card: Card) -> Option<PathBuf> {
        let dir = self.directory.as_ref()?;
        let path = dir.join(card.face_file_name());
        path.is_file().then_some(path)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Theme {
    pub name: String,
    pub directory: PathBuf,
    pub table: Tablecloth,
    pub back: CardBack,
    pub faces: CardFaces,
}

impl Theme {
    pub fn builtin() -> Self {
        Self {
            name: "Clásico".to_string(),
            directory: PathBuf::new(),
            table: Tablecloth {
                color: Rgb::new(0x14, 0x6C, 0x43),
                image: None,
            },
            back: CardBack {
                color: Rgb::new(0x1B, 0x3A, 0x6B),
                pattern: BackPattern::Diamond,
                image: None,
            },
            faces: CardFaces {
                style: FaceStyle::Classic,
                directory: None,
            },
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ThemeError {
    #[error("no se pudo leer {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("tema inválido en {path}: {source}")]
    Parse {
        path: PathBuf,
        source: toml::de::Error,
    },
    #[error("color inválido '{value}' en {field} (usa #RRGGBB)")]
    Color { field: &'static str, value: String },
    #[error("back_pattern inválido '{0}' (solid, diamond o lines)")]
    Pattern(String),
    #[error("face_style inválido '{0}'")]
    FaceStyle(String),
}

#[derive(Debug, Deserialize)]
struct ThemeFile {
    name: String,
    table_color: String,
    table_image: Option<String>,
    back_color: String,
    back_pattern: String,
    back_image: Option<String>,
    face_style: String,
    faces_dir: Option<String>,
}

pub fn load_theme(path: &Path) -> Result<Theme, ThemeError> {
    let text = fs::read_to_string(path).map_err(|source| ThemeError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    let file: ThemeFile = toml::from_str(&text).map_err(|source| ThemeError::Parse {
        path: path.to_path_buf(),
        source,
    })?;
    let directory = path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();
    Ok(Theme {
        name: file.name,
        directory: directory.clone(),
        table: Tablecloth {
            color: parse_rgb(&file.table_color, "table_color")?,
            image: optional_file(&directory, file.table_image),
        },
        back: CardBack {
            color: parse_rgb(&file.back_color, "back_color")?,
            pattern: parse_pattern(&file.back_pattern)?,
            image: optional_file(&directory, file.back_image),
        },
        faces: CardFaces {
            style: parse_face_style(&file.face_style)?,
            directory: file.faces_dir.map(|name| directory.join(name)),
        },
    })
}

fn optional_file(directory: &Path, name: Option<String>) -> Option<PathBuf> {
    name.map(|name| directory.join(name))
}

fn parse_rgb(value: &str, field: &'static str) -> Result<Rgb, ThemeError> {
    let hex = value.trim().trim_start_matches('#');
    if hex.len() != 6 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(ThemeError::Color {
            field,
            value: value.to_string(),
        });
    }
    let channel = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).unwrap_or(0);
    Ok(Rgb::new(channel(0), channel(2), channel(4)))
}

fn parse_pattern(value: &str) -> Result<BackPattern, ThemeError> {
    match value.trim() {
        "solid" => Ok(BackPattern::Solid),
        "diamond" => Ok(BackPattern::Diamond),
        "lines" => Ok(BackPattern::Lines),
        other => Err(ThemeError::Pattern(other.to_string())),
    }
}

fn parse_face_style(value: &str) -> Result<FaceStyle, ThemeError> {
    match value.trim() {
        "classic" => Ok(FaceStyle::Classic),
        "grande" => Ok(FaceStyle::Grande),
        "four" => Ok(FaceStyle::FourColor),
        "night" => Ok(FaceStyle::Night),
        other => Err(ThemeError::FaceStyle(other.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Rank, Suit};

    #[test]
    fn loads_classic_theme_with_fallbacks() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../themes/clasico/theme.toml");
        let theme = load_theme(&path).unwrap();
        assert_eq!(theme.name, "Clásico");
        assert_eq!(theme.table.color, Rgb::new(0x14, 0x6C, 0x43));
        assert!(theme.table.image.is_none());
        assert_eq!(theme.back.pattern, BackPattern::Diamond);
        assert!(theme.faces.directory.is_none());
        assert!(theme
            .faces
            .image_for(Card::new(Suit::Hearts, Rank::Ace))
            .is_none());
    }

    #[test]
    fn face_file_is_used_only_when_present() {
        let dir = std::env::temp_dir().join(format!("cualsol-faces-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let card = Card::new(Suit::Spades, Rank::Ten);
        fs::write(dir.join(card.face_file_name()), b"png").unwrap();
        let faces = CardFaces {
            style: FaceStyle::Classic,
            directory: Some(dir.clone()),
        };
        assert_eq!(
            faces.image_for(card).as_deref(),
            Some(dir.join("spades_10.png").as_path())
        );
        assert!(faces
            .image_for(Card::new(Suit::Hearts, Rank::Ace))
            .is_none());
        let _ = fs::remove_dir_all(&dir);
    }
}
