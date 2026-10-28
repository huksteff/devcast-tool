//! Codes of mouse buttons and cursor kinds in `cursor.bin`.

/// A mouse button.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Button {
    Left,
    Right,
    Middle,
    /// The «back» side button.
    X1,
    /// The «forward» side button.
    X2,
}

/// What the cursor is: a standard system cursor (Windows `IDC_*`) or the application's own.
/// The discriminant is the code in the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum CursorKind {
    Arrow,
    /// I-beam over text.
    Text,
    /// The hand over links.
    Pointer,
    Crosshair,
    /// Resize left–right.
    ResizeEw,
    /// Resize up–down.
    ResizeNs,
    /// Resize along the diagonal from top-left to bottom-right.
    ResizeNwse,
    /// Resize along the diagonal from top-right to bottom-left.
    ResizeNesw,
    /// Four arrows: move.
    Move,
    NotAllowed,
    /// Busy: the hourglass or circle alone.
    Wait,
    /// Working in the background: arrow with a circle.
    Progress,
    /// Arrow with a question mark.
    Help,
    /// Not a standard cursor: drawn by the application.
    Custom,
}

impl Button {
    pub(crate) fn code(self) -> u8 {
        match self {
            Self::Left => 1,
            Self::Right => 2,
            Self::Middle => 3,
            Self::X1 => 4,
            Self::X2 => 5,
        }
    }

    pub(crate) fn from_code(code: u8) -> Result<Self, String> {
        Ok(match code {
            1 => Self::Left,
            2 => Self::Right,
            3 => Self::Middle,
            4 => Self::X1,
            5 => Self::X2,
            _ => return Err(format!("неизвестная кнопка мыши {code}")),
        })
    }
}

impl CursorKind {
    /// Every kind, in code order (code = index).
    pub const ALL: [Self; 14] = [
        Self::Arrow,
        Self::Text,
        Self::Pointer,
        Self::Crosshair,
        Self::ResizeEw,
        Self::ResizeNs,
        Self::ResizeNwse,
        Self::ResizeNesw,
        Self::Move,
        Self::NotAllowed,
        Self::Wait,
        Self::Progress,
        Self::Help,
        Self::Custom,
    ];

    pub(crate) fn code(self) -> u8 {
        self as u8
    }

    pub(crate) fn from_code(code: u8) -> Result<Self, String> {
        Self::ALL
            .get(usize::from(code))
            .copied()
            .ok_or_else(|| format!("неизвестный тип курсора {code}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_round_trip() {
        for (code, kind) in CursorKind::ALL.iter().enumerate() {
            assert_eq!(kind.code(), code as u8);
            assert_eq!(CursorKind::from_code(code as u8), Ok(*kind));
        }
        assert!(CursorKind::from_code(14).is_err());
    }
}
