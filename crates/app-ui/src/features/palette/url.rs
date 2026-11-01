//! What an address of the app's own scheme may ask. Anything that can open a link (a web page, a message, a script, Shortcuts)
//! can open `figghost://...`, so only a few things that do no harm are on offer: show the palette, a page of the settings, a
//! tab of the notch, go to the agent that waits, and add a reminder. Nothing that changes a setting, stops a program or sends
//! data is.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Palette,
    /// A page of the settings window.
    Page(String),
    /// A tab of the notch.
    Tab(String),
    /// The terminal of the agent that has waited the longest.
    Jump,
    /// A reminder, written the way a person says it.
    Remind(String),
}

/// The most characters of a reminder that an address may carry.
pub const MAX_REMINDER: usize = 200;

fn id(text: &str) -> Option<String> {
    (!text.is_empty()
        && text.len() <= 24
        && text
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_'))
    .then(|| text.to_owned())
}

/// The text of a query value: `%20` and `+` read as a space and other `%` escapes as their bytes; a value that is not
/// UTF-8 or holds a control character is refused.
fn decode(value: &str) -> Option<String> {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => out.push(b' '),
            b'%' => {
                let pair = value.get(i + 1..i + 3)?;
                out.push(u8::from_str_radix(pair, 16).ok()?);
                i += 2;
            }
            b => out.push(b),
        }
        i += 1;
    }
    let text = String::from_utf8(out).ok()?;
    (!text.chars().any(char::is_control)).then_some(text)
}

pub fn parse(url: &str) -> Option<Command> {
    let rest = url.strip_prefix("figghost://")?;
    let (path, query) = rest.split_once('?').unwrap_or((rest, ""));
    let path = path.trim_end_matches('/');
    let mut parts = path.split('/');
    let head = parts.next()?;
    let tail = parts.next();
    if parts.next().is_some() {
        return None;
    }
    match (head, tail) {
        ("palette", None) => Some(Command::Palette),
        ("page", Some(page)) => Some(Command::Page(id(page)?)),
        ("tab", Some(tab)) => Some(Command::Tab(id(tab)?)),
        ("agents", Some("jump")) => Some(Command::Jump),
        ("remind", None) => {
            let text = query
                .split('&')
                .find_map(|pair| pair.strip_prefix("text="))?;
            let text = decode(text)?;
            let text = text.trim();
            (!text.is_empty() && text.chars().count() <= MAX_REMINDER)
                .then(|| Command::Remind(text.to_owned()))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_few_things_an_address_may_ask_are_read() {
        assert_eq!(parse("figghost://palette"), Some(Command::Palette));
        assert_eq!(parse("figghost://palette/"), Some(Command::Palette));
        assert_eq!(
            parse("figghost://page/network"),
            Some(Command::Page("network".into()))
        );
        assert_eq!(
            parse("figghost://tab/pomodoro"),
            Some(Command::Tab("pomodoro".into()))
        );
        assert_eq!(parse("figghost://agents/jump"), Some(Command::Jump));
        assert_eq!(
            parse("figghost://remind?text=call%20mom+tomorrow%209h"),
            Some(Command::Remind("call mom tomorrow 9h".into()))
        );
        assert_eq!(
            parse("figghost://remind?x=1&text=n%E1%BB%99p%20b%C3%A1o%20c%C3%A1o"),
            Some(Command::Remind("n\u{1ed9}p b\u{e1}o c\u{e1}o".into()))
        );
    }

    #[test]
    fn whatever_else_is_refused() {
        for bad in [
            "",
            "http://palette",
            "figghost:/palette",
            "figghost://",
            "figghost://quit",
            "figghost://settings/set?path=network.control&value=true",
            "figghost://page/Network",
            "figghost://page/../x",
            "figghost://page/a/b",
            "figghost://page/",
            "figghost://tab/",
            "figghost://page/aaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "figghost://agents/stop",
            "figghost://remind",
            "figghost://remind?text=",
            "figghost://remind?text=%20%20",
            "figghost://remind?text=a%0Ab",
            "figghost://remind?text=%FF%FE",
            "figghost://remind?text=%zz",
            "figghost://remind?text=%4",
        ] {
            assert_eq!(parse(bad), None, "{bad:?}");
        }
        let long = format!("figghost://remind?text={}", "a".repeat(MAX_REMINDER + 1));
        assert_eq!(parse(&long), None);
        let ok = format!("figghost://remind?text={}", "a".repeat(MAX_REMINDER));
        assert!(parse(&ok).is_some());
    }
}
