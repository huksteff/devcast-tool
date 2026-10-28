//! What to record: a whole monitor, one window or an area of the desktop.
//!
//! Text form (CLI and files): `screen:<n>`, `window:<id>`, `area:<x>,<y>,<w>,<h>`.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// A capture source. Stored as its text form, e.g. `"screen:1"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub enum Source {
    /// Monitor number, starting from 1, in the order the system lists monitors.
    Monitor(u32),
    /// Window handle as a number; valid while the window exists.
    Window(u64),
    /// Desktop rectangle in physical pixels; must lie inside one monitor.
    Area(Area),
}

/// Rectangle on the virtual desktop in physical pixels: `x`, `y` of the top-left corner
/// (negative left of / above the primary monitor), size never zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Area {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

impl Area {
    pub fn new(x: i32, y: i32, width: u32, height: u32) -> Result<Self, SourceError> {
        if width == 0 || height == 0 {
            return Err(SourceError::EmptyArea { width, height });
        }
        Ok(Self {
            x,
            y,
            width,
            height,
        })
    }

    pub const fn x(&self) -> i32 {
        self.x
    }
    pub const fn y(&self) -> i32 {
        self.y
    }
    pub const fn width(&self) -> u32 {
        self.width
    }
    pub const fn height(&self) -> u32 {
        self.height
    }
}

/// Text that is not a source, or a source with impossible values.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SourceError {
    #[error(
        "«{0}» — не источник записи: ожидается screen:<номер>, window:<id> или area:x,y,w,h (например screen:1, area:0,0,1280,720)"
    )]
    Format(String),
    #[error("мониторы нумеруются с 1: укажи screen:1 или больше (список — devcast ls sources)")]
    MonitorZero,
    #[error("у области нулевой размер {width}×{height}: ширина и высота должны быть больше 0")]
    EmptyArea { width: u32, height: u32 },
}

impl FromStr for Source {
    type Err = SourceError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let format = || SourceError::Format(text.to_owned());
        let (kind, value) = text.split_once(':').ok_or_else(format)?;
        match kind.trim() {
            "screen" => match value.trim().parse().map_err(|_| format())? {
                0 => Err(SourceError::MonitorZero),
                n => Ok(Self::Monitor(n)),
            },
            "window" => Ok(Self::Window(value.trim().parse().map_err(|_| format())?)),
            "area" => {
                let parts: Vec<&str> = value.split(',').map(str::trim).collect();
                let [x, y, w, h] = parts[..] else {
                    return Err(format());
                };
                let pos = |s: &str| s.parse::<i32>().map_err(|_| format());
                let size = |s: &str| s.parse::<u32>().map_err(|_| format());
                Ok(Self::Area(Area::new(pos(x)?, pos(y)?, size(w)?, size(h)?)?))
            }
            _ => Err(format()),
        }
    }
}

impl fmt::Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Monitor(n) => write!(f, "screen:{n}"),
            Self::Window(id) => write!(f, "window:{id}"),
            Self::Area(a) => write!(f, "area:{},{},{},{}", a.x, a.y, a.width, a.height),
        }
    }
}

impl TryFrom<String> for Source {
    type Error = SourceError;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        text.parse()
    }
}

impl From<Source> for String {
    fn from(source: Source) -> Self {
        source.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_round_trips() {
        let sources = [
            Source::Monitor(2),
            Source::Window(263_456),
            Source::Area(Area::new(-1700, 150, 1280, 720).unwrap()),
        ];
        for source in sources {
            let text = source.to_string();
            assert_eq!(text.parse::<Source>().unwrap(), source, "{text}");
            let json = serde_json::to_string(&source).unwrap();
            assert_eq!(json, format!("\"{text}\""));
            assert_eq!(serde_json::from_str::<Source>(&json).unwrap(), source);
        }
        assert_eq!(
            " area: 1, 2 ,3,4".parse::<Source>().unwrap(),
            Source::Area(Area::new(1, 2, 3, 4).unwrap())
        );
    }

    #[test]
    fn bad_text_explains_the_form() {
        for text in [
            "",
            "screen",
            "screen:x",
            "screen:-1",
            "monitor:1",
            "window:",
            "area:1,2,3",
            "area:1,2,3,4,5",
            "area:1,2,-3,4",
        ] {
            let error = text.parse::<Source>().unwrap_err();
            assert!(matches!(error, SourceError::Format(_)), "{text}: {error}");
            assert!(error.to_string().contains("screen:<номер>"), "{error}");
        }
    }

    #[test]
    fn rejects_impossible_values() {
        assert_eq!(
            "screen:0".parse::<Source>().unwrap_err(),
            SourceError::MonitorZero
        );
        let error = "area:0,0,0,720".parse::<Source>().unwrap_err();
        assert_eq!(
            error,
            SourceError::EmptyArea {
                width: 0,
                height: 720
            }
        );
        let error = serde_json::from_str::<Source>(r#""area:0,0,10,0""#).unwrap_err();
        assert!(error.to_string().contains("нулевой размер"), "{error}");
    }
}
