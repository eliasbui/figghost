//! The words of the Commands tab, in each language.

use crate::i18n::Language;

fn pick(language: Language, en: &str, vi: &str) -> String {
    match language {
        Language::English => en.to_owned(),
        Language::Vietnamese => vi.to_owned(),
    }
}

pub fn tab(language: Language) -> String {
    pick(language, "Commands", "L\u{1ec7}nh")
}

pub fn caption(language: Language) -> String {
    pick(
        language,
        "Saved commands",
        "L\u{1ec7}nh \u{111}\u{e3} l\u{1b0}u",
    )
}

pub fn stop(language: Language) -> String {
    pick(language, "Stop", "D\u{1eeb}ng")
}

pub fn clear(language: Language) -> String {
    pick(language, "Clear the output", "X\u{f3}a \u{111}\u{1ea7}u ra")
}

pub fn placeholder(language: Language) -> String {
    pick(
        language,
        "Press a command to run it.",
        "B\u{1ea5}m v\u{e0}o m\u{1ed9}t l\u{1ec7}nh \u{111}\u{1ec3} ch\u{1ea1}y.",
    )
}

pub fn off(language: Language) -> (String, String) {
    (
        pick(
            language,
            "Commands is off",
            "L\u{1ec7}nh \u{111}ang t\u{1eaf}t",
        ),
        pick(
            language,
            "Turn it on in Settings, Commands.",
            "B\u{1ead}t trong C\u{e0}i \u{111}\u{1eb7}t, L\u{1ec7}nh.",
        ),
    )
}

pub fn none(language: Language) -> (String, String) {
    (
        pick(
            language,
            "No command saved yet",
            "Ch\u{1b0}a l\u{1b0}u l\u{1ec7}nh n\u{e0}o",
        ),
        pick(
            language,
            "Add one in Settings, Commands: a name and the line to run.",
            "Th\u{ea}m trong C\u{e0}i \u{111}\u{1eb7}t, L\u{1ec7}nh: m\u{1ed9}t c\u{e1}i t\u{ea}n v\u{e0} d\u{f2}ng l\u{1ec7}nh c\u{1ea7}n ch\u{1ea1}y.",
        ),
    )
}

pub fn running(language: Language, name: &str) -> String {
    match language {
        Language::English => format!("{name} is running"),
        Language::Vietnamese => format!("{name} \u{111}ang ch\u{1ea1}y"),
    }
}

pub fn finished(language: Language, name: &str, code: i32) -> String {
    match (language, code) {
        (Language::English, 0) => format!("{name} finished"),
        (Language::English, n) => format!("{name} ended with {n}"),
        (Language::Vietnamese, 0) => format!("{name} \u{111}\u{e3} xong"),
        (Language::Vietnamese, n) => format!("{name} k\u{1ebf}t th\u{fa}c v\u{1edb}i m\u{e3} {n}"),
    }
}

pub fn stopped(language: Language, name: &str) -> String {
    match language {
        Language::English => format!("{name} was stopped"),
        Language::Vietnamese => format!("{name} \u{111}\u{e3} b\u{1ecb} d\u{1eeb}ng"),
    }
}

pub fn too_long(language: Language, name: &str) -> String {
    match language {
        Language::English => format!("{name} ran too long and was stopped"),
        Language::Vietnamese => format!(
            "{name} ch\u{1ea1}y qu\u{e1} l\u{e2}u n\u{ea}n \u{111}\u{e3} b\u{1ecb} d\u{1eeb}ng"
        ),
    }
}

pub fn failed(language: Language, name: &str) -> String {
    match language {
        Language::English => format!("{name} could not be started"),
        Language::Vietnamese => format!("Kh\u{f4}ng ch\u{1ea1}y \u{111}\u{1b0}\u{1ee3}c {name}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_word_is_said_in_both_languages() {
        let (en, vi) = (Language::English, Language::Vietnamese);
        for (a, b) in [
            (tab(en), tab(vi)),
            (caption(en), caption(vi)),
            (stop(en), stop(vi)),
            (clear(en), clear(vi)),
            (placeholder(en), placeholder(vi)),
            (off(en).0, off(vi).0),
            (none(en).1, none(vi).1),
            (running(en, "x"), running(vi, "x")),
            (finished(en, "x", 0), finished(vi, "x", 0)),
            (finished(en, "x", 2), finished(vi, "x", 2)),
            (stopped(en, "x"), stopped(vi, "x")),
            (too_long(en, "x"), too_long(vi, "x")),
            (failed(en, "x"), failed(vi, "x")),
        ] {
            assert!(!a.is_empty() && !b.is_empty() && a != b, "{a} / {b}");
        }
        assert!(
            finished(en, "Build", 3).contains('3') && finished(en, "Build", 3).contains("Build")
        );
    }
}
