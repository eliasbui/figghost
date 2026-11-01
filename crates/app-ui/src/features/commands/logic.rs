//! The commands the person saved, and what they print: who may be saved, and how the output is cleaned for a plain page.

/// How many commands the tab has room for.
pub const MAX_ITEMS: usize = 8;
const MAX_NAME: usize = 40;
const MAX_COMMAND: usize = 500;
/// The most of the output that is kept; the oldest is let go.
pub const MAX_OUTPUT_CHARS: usize = 200_000;

/// A saved command: what it is called, and the line given to the shell.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Item {
    pub name: String,
    pub command: String,
}

fn plain(text: &str, most: usize) -> Option<String> {
    let text = text.trim();
    (!text.is_empty() && text.chars().count() <= most && !text.chars().any(char::is_control))
        .then(|| text.to_owned())
}

impl Item {
    /// The item as it may be kept: both parts of the right shape, or `None`.
    pub fn valid(&self) -> Option<Item> {
        Some(Item {
            name: plain(&self.name, MAX_NAME)?,
            command: plain(&self.command, MAX_COMMAND)?,
        })
    }
}

/// The items that can be kept, in order, no more than the tab has room for.
pub fn sanitized(items: &[Item]) -> Vec<Item> {
    items
        .iter()
        .filter_map(Item::valid)
        .take(MAX_ITEMS)
        .collect()
}

/// Takes the colour and cursor codes out of what a program printed, and makes line ends plain, across the pieces it is
/// printed in: a code cut in two by the end of a piece is finished by the next.
#[derive(Debug, Default)]
pub struct Cleaner {
    state: State,
    /// A carriage return that is waiting to learn whether a line feed follows.
    cr: bool,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum State {
    #[default]
    Text,
    /// After ESC.
    Escape,
    /// Inside `ESC [ ... final`.
    Csi,
    /// Inside `ESC ] ...` up to BEL or `ESC \`.
    Osc,
    /// Inside an OSC, after an ESC.
    OscEscape,
    /// After `ESC` and one of `( ) # %`, which take one more character.
    Charset,
}

impl Cleaner {
    pub fn feed(&mut self, piece: &str) -> String {
        let mut out = String::with_capacity(piece.len());
        for c in piece.chars() {
            if self.cr && c != '\n' && self.state == State::Text {
                // A bare carriage return: a program redrawing its line. It is a new line here.
                out.push('\n');
            }
            self.cr = false;
            match self.state {
                State::Text => match c {
                    '\u{1b}' => self.state = State::Escape,
                    '\r' => self.cr = true,
                    '\n' | '\t' => out.push(c),
                    c if c.is_control() => {}
                    c => out.push(c),
                },
                State::Escape => {
                    self.state = match c {
                        '[' => State::Csi,
                        ']' => State::Osc,
                        '(' | ')' | '#' | '%' => State::Charset,
                        _ => State::Text,
                    }
                }
                State::Csi => {
                    if ('\u{40}'..='\u{7e}').contains(&c) {
                        self.state = State::Text;
                    }
                }
                State::Osc => match c {
                    '\u{7}' => self.state = State::Text,
                    '\u{1b}' => self.state = State::OscEscape,
                    _ => {}
                },
                State::OscEscape => self.state = if c == '\\' { State::Text } else { State::Osc },
                State::Charset => self.state = State::Text,
            }
        }
        out
    }
}

/// `text` with its oldest characters let go so that no more than `most` are left, cut at a line end when there is one.
pub fn tail(text: &str, most: usize) -> &str {
    let count = text.chars().count();
    if count <= most {
        return text;
    }
    let start = text.char_indices().nth(count - most).map_or(0, |(i, _)| i);
    let rest = &text[start..];
    rest.split_once('\n').map_or(rest, |(_, after)| after)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(name: &str, command: &str) -> Item {
        Item {
            name: name.into(),
            command: command.into(),
        }
    }

    #[test]
    fn an_item_needs_a_name_and_a_command_of_plain_text() {
        assert_eq!(
            item(" Build ", " cargo build ").valid(),
            Some(item("Build", "cargo build"))
        );
        for bad in [
            item("", "ls"),
            item("x", ""),
            item("  ", "ls"),
            item("a\nb", "ls"),
            item("x", "ls\nrm -rf x"),
            item("x", "ls\u{7}"),
        ] {
            assert_eq!(bad.valid(), None, "{bad:?}");
        }
        assert_eq!(item(&"n".repeat(41), "ls").valid(), None);
        assert_eq!(
            item(&"n".repeat(40), &"c".repeat(500))
                .valid()
                .map(|i| i.command.len()),
            Some(500)
        );
        assert_eq!(item("x", &"c".repeat(501)).valid(), None);
        let many: Vec<Item> = (0..20).map(|i| item(&format!("c{i}"), "ls")).collect();
        let kept = sanitized(&[many, vec![item("", "x")]].concat());
        assert_eq!(kept.len(), MAX_ITEMS);
        assert_eq!(kept[0].name, "c0", "the order is kept");
    }

    #[test]
    fn colours_and_cursor_codes_are_taken_out_and_the_words_stay() {
        let mut c = Cleaner::default();
        assert_eq!(c.feed("\u{1b}[1;32mBuild\u{1b}[0m ok\n"), "Build ok\n");
        assert_eq!(c.feed("\u{1b}]0;title\u{7}after\n"), "after\n");
        assert_eq!(
            c.feed("\u{1b}]8;;http://x\u{1b}\\link\u{1b}]8;;\u{1b}\\\n"),
            "link\n"
        );
        assert_eq!(c.feed("\u{1b}(Bplain\n"), "plain\n");
        assert_eq!(
            c.feed("tab\there\u{7}\u{8}\n"),
            "tab\there\n",
            "bells and backspaces are dropped"
        );
        assert_eq!(
            c.feed("caf\u{e9} \u{1ee9}\n"),
            "caf\u{e9} \u{1ee9}\n",
            "words in any language stay"
        );
    }

    #[test]
    fn a_code_cut_between_two_pieces_is_finished_by_the_next() {
        let mut c = Cleaner::default();
        let whole = format!(
            "{}{}{}",
            c.feed("red \u{1b}[3"),
            c.feed("1mtext\u{1b}"),
            c.feed("[0m done")
        );
        assert_eq!(whole, "red text done");
    }

    #[test]
    fn a_carriage_return_is_a_line_end_but_a_pair_with_a_line_feed_is_one() {
        let mut c = Cleaner::default();
        assert_eq!(c.feed("one\r\ntwo\rthree\n"), "one\ntwo\nthree\n");
        let mut c = Cleaner::default();
        let whole = format!("{}{}", c.feed("a\r"), c.feed("\nb"));
        assert_eq!(
            whole, "a\nb",
            "the pair is split between two pieces and is still one line end"
        );
    }

    #[test]
    fn the_output_keeps_its_end_and_is_cut_at_a_line() {
        assert_eq!(tail("short", 100), "short");
        let text = "aaaa\nbbbb\ncccc\n";
        assert_eq!(
            tail(text, 8),
            "cccc\n",
            "cut at the line that follows the cut"
        );
        assert_eq!(
            tail("x".repeat(50).as_str(), 10),
            "x".repeat(10),
            "no line end to cut at"
        );
        assert_eq!(
            tail("\u{1ee9}\u{1ee9}\u{1ee9}\u{1ee9}", 2).chars().count(),
            2
        );
    }
}
