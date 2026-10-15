//! Names of pressed keys for the key display of a recording: "Cmd + Shift + 4", "Return".

/// Modifier keys held with a key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Modifiers {
    pub control: bool,
    pub option: bool,
    pub shift: bool,
    pub command: bool,
}

impl Modifiers {
    /// From CGEvent flags.
    pub const fn from_flags(flags: u64) -> Self {
        Self {
            shift: flags & 0x0002_0000 != 0,
            control: flags & 0x0004_0000 != 0,
            option: flags & 0x0008_0000 != 0,
            command: flags & 0x0010_0000 != 0,
        }
    }

    /// Held for a shortcut rather than for typing a capital or a symbol.
    const fn makes_shortcut(self) -> bool {
        self.command || self.control || self.option
    }
}

/// The key's name on a US keyboard layout, from its virtual key code.
pub fn key_name(code: u16) -> Option<&'static str> {
    Some(match code {
        0 => "A",
        11 => "B",
        8 => "C",
        2 => "D",
        14 => "E",
        3 => "F",
        5 => "G",
        4 => "H",
        34 => "I",
        38 => "J",
        40 => "K",
        37 => "L",
        46 => "M",
        45 => "N",
        31 => "O",
        35 => "P",
        12 => "Q",
        15 => "R",
        1 => "S",
        17 => "T",
        32 => "U",
        9 => "V",
        13 => "W",
        7 => "X",
        16 => "Y",
        6 => "Z",
        18 => "1",
        19 => "2",
        20 => "3",
        21 => "4",
        23 => "5",
        22 => "6",
        26 => "7",
        28 => "8",
        25 => "9",
        29 => "0",
        27 => "-",
        24 => "=",
        33 => "[",
        30 => "]",
        42 => "\\",
        41 => ";",
        39 => "'",
        43 => ",",
        47 => ".",
        44 => "/",
        50 => "`",
        36 | 76 => "Return",
        48 => "Tab",
        49 => "Space",
        51 => "Delete",
        117 => "Forward Delete",
        53 => "Esc",
        123 => "Left",
        124 => "Right",
        125 => "Down",
        126 => "Up",
        115 => "Home",
        119 => "End",
        116 => "Page Up",
        121 => "Page Down",
        122 => "F1",
        120 => "F2",
        99 => "F3",
        118 => "F4",
        96 => "F5",
        97 => "F6",
        98 => "F7",
        100 => "F8",
        101 => "F9",
        109 => "F10",
        103 => "F11",
        111 => "F12",
        _ => return None,
    })
}

/// Keys worth showing even without a modifier: they act rather than type.
fn acts_alone(code: u16) -> bool {
    matches!(
        code,
        36 | 76 | 48 | 51 | 117 | 53 | 115 | 116 | 119 | 121 | 123..=126 | 96..=101 | 103 | 109 | 111 | 118 | 120 | 122
    )
}

/// What the key display shows for a key press, or `None` when it stays hidden. Plain typing (letters,
/// digits, symbols, with or without Shift) is not shown, so a password typed during a recording never
/// appears in it.
pub fn label(code: u16, modifiers: Modifiers) -> Option<String> {
    let name = key_name(code)?;
    if !modifiers.makes_shortcut() && !acts_alone(code) {
        return None;
    }
    let mut parts = Vec::with_capacity(5);
    for (held, text) in [
        (modifiers.control, "Ctrl"),
        (modifiers.option, "Opt"),
        (modifiers.shift, "Shift"),
        (modifiers.command, "Cmd"),
    ] {
        if held {
            parts.push(text);
        }
    }
    parts.push(name);
    Some(parts.join(" + "))
}

#[cfg(test)]
mod tests {
    use super::*;

    const COMMAND_SHIFT: Modifiers = Modifiers {
        control: false,
        option: false,
        shift: true,
        command: true,
    };

    #[test]
    fn shortcuts_read_like_the_menus_write_them() {
        assert_eq!(label(21, COMMAND_SHIFT).as_deref(), Some("Shift + Cmd + 4"));
        let all = Modifiers {
            control: true,
            option: true,
            shift: true,
            command: true,
        };
        assert_eq!(
            label(1, all).as_deref(),
            Some("Ctrl + Opt + Shift + Cmd + S")
        );
    }

    #[test]
    fn plain_typing_stays_hidden() {
        assert_eq!(label(0, Modifiers::default()), None);
        let shift = Modifiers {
            shift: true,
            ..Modifiers::default()
        };
        assert_eq!(label(18, shift), None, "a typed '!' is not a shortcut");
    }

    #[test]
    fn keys_that_act_show_on_their_own() {
        assert_eq!(label(36, Modifiers::default()).as_deref(), Some("Return"));
        assert_eq!(label(53, Modifiers::default()).as_deref(), Some("Esc"));
        assert_eq!(label(123, Modifiers::default()).as_deref(), Some("Left"));
        assert_eq!(label(999, COMMAND_SHIFT), None);
    }

    #[test]
    fn flags_decode() {
        assert_eq!(Modifiers::from_flags(0x0012_0000), COMMAND_SHIFT);
        assert_eq!(Modifiers::from_flags(0), Modifiers::default());
    }
}
