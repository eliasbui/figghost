//! Everything Smart Select says in the native window, in both languages. The Settings page has its own strings
//! in `web/features/smart.js`.

use crate::i18n::Language;
use core_smart::{translate::Failure, units::Dim};

impl Language {
    pub fn smart_menu(self) -> &'static str {
        match self {
            Language::English => "Smart Select",
            Language::Vietnamese => "Ch\u{1ecd}n th\u{f4}ng minh",
        }
    }

    pub fn smart_footer(self) -> &'static str {
        match self {
            Language::English => "Return copies, Esc closes",
            Language::Vietnamese => {
                "Enter \u{111}\u{1ec3} sao ch\u{e9}p, Esc \u{111}\u{1ec3} \u{111}\u{f3}ng"
            }
        }
    }

    pub fn smart_footer_message(self) -> &'static str {
        match self {
            Language::English => "Esc closes",
            Language::Vietnamese => "Esc \u{111}\u{1ec3} \u{111}\u{f3}ng",
        }
    }

    /// The footer of a list of amounts: which day the rates are from, then the keys.
    pub fn smart_footer_rates(self, date: &str) -> String {
        match self {
            Language::English => format!("Rates of {date}. {}", self.smart_footer()),
            Language::Vietnamese => format!(
                "T\u{1ef7} gi\u{e1} ng\u{e0}y {date}. {}",
                self.smart_footer()
            ),
        }
    }

    pub fn smart_nothing_selected(self) -> &'static str {
        match self {
            Language::English => "Select some text first, then try again.",
            Language::Vietnamese => {
                "H\u{e3}y ch\u{1ecd}n m\u{1ed9}t \u{111}o\u{1ea1}n v\u{103}n b\u{1ea3}n tr\u{1b0}\u{1edb}c r\u{1ed3}i th\u{1ed1} l\u{1ea1}i."
            }
        }
    }

    pub fn smart_needs_accessibility(self) -> &'static str {
        match self {
            Language::English => {
                "Smart Select needs Accessibility access to read the selected text. Turn it on in System Settings, Privacy and Security."
            }
            Language::Vietnamese => {
                "Smart Select c\u{1ea7}n quy\u{1ec1}n Tr\u{1ee3} n\u{103}ng \u{111}\u{1ec3} \u{111}\u{1ecd}c \u{111}o\u{1ea1}n v\u{103}n b\u{1ea3}n \u{111}ang ch\u{1ecd}n. H\u{e3}y b\u{1ead}t trong C\u{e0}i \u{111}\u{1eb7}t h\u{1ec7} th\u{1ed1}ng, Quy\u{1ec1}n ri\u{ea}ng t\u{1b0} v\u{e0} B\u{1ea3}o m\u{1ead}t."
            }
        }
    }

    pub fn smart_clipboard_not_restorable(self) -> &'static str {
        match self {
            Language::English => {
                "This app does not share its selection, and the clipboard holds something that cannot be put back, so nothing was copied. Copy the text yourself and try again."
            }
            Language::Vietnamese => {
                "\u{1ee8}ng d\u{1ee5}ng n\u{e0}y kh\u{f4}ng chia s\u{1ebb} ph\u{1ea7}n \u{111}ang ch\u{1ecd}n v\u{e0} clipboard \u{111}ang ch\u{1ee9}a d\u{1eef} li\u{1ec7}u kh\u{f4}ng th\u{1ec3} kh\u{f4}i ph\u{1ee5}c, n\u{ea}n kh\u{f4}ng sao ch\u{e9}p g\u{EC}. H\u{e3}y t\u{1ef1} sao ch\u{e9}p v\u{e0} th\u{1eed} l\u{1ea1}i."
            }
        }
    }

    pub fn smart_internet_for_translation(self) -> &'static str {
        match self {
            Language::English => {
                "Turn on the internet in the Smart Select settings to translate this text."
            }
            Language::Vietnamese => {
                "H\u{e3}y b\u{1ead}t k\u{1ebf}t n\u{1ed1}i internet trong c\u{e0}i \u{111}\u{1eb7}t Smart Select \u{111}\u{1ec3} d\u{1ecb}ch \u{111}o\u{1ea1}n v\u{103}n b\u{1ea3}n n\u{e0}y."
            }
        }
    }

    pub fn smart_internet_for_rates(self) -> &'static str {
        match self {
            Language::English => {
                "Turn on the internet in the Smart Select settings to convert currencies."
            }
            Language::Vietnamese => {
                "H\u{e3}y b\u{1ead}t k\u{1ebf}t n\u{1ed1}i internet trong c\u{e0}i \u{111}\u{1eb7}t Smart Select \u{111}\u{1ec3} \u{111}\u{1ed5}i ti\u{1ec1}n t\u{1ec7}."
            }
        }
    }

    pub fn smart_fetching_rates(self) -> &'static str {
        match self {
            Language::English => "Getting the exchange rates...",
            Language::Vietnamese => "\u{110}ang l\u{1ea5}y t\u{1ef7} gi\u{e1}...",
        }
    }

    pub fn smart_rates_unavailable(self) -> &'static str {
        match self {
            Language::English => "The exchange rates are not available right now. Try again later.",
            Language::Vietnamese => {
                "Hi\u{1ec7}n kh\u{f4}ng l\u{1ea5}y \u{111}\u{1b0}\u{1ee3}c t\u{1ef7} gi\u{e1}. H\u{e3}y th\u{1eed} l\u{1ea1}i sau."
            }
        }
    }

    pub fn smart_nothing_to_show(self) -> &'static str {
        match self {
            Language::English => {
                "There is nothing to show it in. Add currencies or time zones in the Smart Select settings."
            }
            Language::Vietnamese => {
                "Kh\u{f4}ng c\u{f3} g\u{EC} \u{111}\u{1ec3} hi\u{1ec3}n th\u{1ecb}. H\u{e3}y th\u{ea}m ti\u{1ec1}n t\u{1ec7} ho\u{1eb7}c m\u{1ee9}i gi\u{1edd} trong c\u{e0}i \u{111}\u{1eb7}t Smart Select."
            }
        }
    }

    pub fn smart_nothing_recognised(self) -> &'static str {
        match self {
            Language::English => {
                "Smart Select understands units, amounts of money, times and, with the internet on, words to translate."
            }
            Language::Vietnamese => {
                "Smart Select hi\u{1ec3}u \u{111}\u{1a1}n v\u{1ecb}, s\u{1ed1} ti\u{1ec1}n, gi\u{1edd} v\u{e0} khi b\u{1ead}t internet th\u{EC} c\u{1ea3} \u{111}o\u{1ea1}n v\u{103}n \u{111}\u{1ec3} d\u{1ecb}ch."
            }
        }
    }

    pub fn smart_translate_header(self, target: &str) -> String {
        match self {
            Language::English => format!("Translate to {target}"),
            Language::Vietnamese => format!("D\u{1ecb}ch sang {target}"),
        }
    }

    pub fn smart_translating(self) -> &'static str {
        match self {
            Language::English => "Translating...",
            Language::Vietnamese => "\u{110}ang d\u{1ecb}ch...",
        }
    }

    pub fn smart_too_long(self, limit: usize) -> String {
        match self {
            Language::English => {
                format!("This text is too long to translate here. The limit is {limit} characters.")
            }
            Language::Vietnamese => format!(
                "\u{110}o\u{1ea1}n v\u{103}n b\u{1ea3}n n\u{e0}y qu\u{e1} d\u{e0}i \u{111}\u{1ec3} d\u{1ecb}ch \u{1edf} \u{111}\u{e2}y. Gi\u{1edb}i h\u{1ea1}n l\u{e0} {limit} k\u{fd} t\u{1ef1}."
            ),
        }
    }

    pub fn smart_translation_failed(self, failure: &TranslationProblem, target: &str) -> String {
        match (self, failure) {
            (Language::English, TranslationProblem::Offline) => {
                "Could not reach the translation service.".into()
            }
            (Language::Vietnamese, TranslationProblem::Offline) => {
                "Kh\u{f4}ng k\u{1ebf}t n\u{1ed1}i \u{111}\u{1b0}\u{1ee3}c v\u{1edb}i d\u{1ecb}ch v\u{1ee5} d\u{1ecb}ch.".into()
            }
            (Language::English, TranslationProblem::Service(Failure::Quota)) => {
                "The free translation allowance for today is used up. Try again later.".into()
            }
            (Language::Vietnamese, TranslationProblem::Service(Failure::Quota)) => {
                "\u{110}\u{e3} h\u{1ebf}t l\u{1b0}\u{1ee3}t d\u{1ecb}ch mi\u{1ec5}n ph\u{ed} h\u{f4}m nay. H\u{e3}y th\u{1eed} l\u{1ea1}i sau.".into()
            }
            (Language::English, TranslationProblem::Service(Failure::SameLanguage)) => {
                format!("This text is already in {target}.")
            }
            (Language::Vietnamese, TranslationProblem::Service(Failure::SameLanguage)) => {
                format!("V\u{103}n b\u{1ea3}n \u{111}\u{e3} \u{1edf} ng\u{f4}n ng\u{1eef} {target}.")
            }
            (Language::English, TranslationProblem::Service(_)) => {
                "The translation service did not give a usable answer.".into()
            }
            (Language::Vietnamese, TranslationProblem::Service(_)) => {
                "D\u{1ecb}ch v\u{1ee5} d\u{1ecb}ch kh\u{f4}ng tr\u{1ea3} v\u{1ec1} k\u{1ebf}t qu\u{1ea3} d\u{f9}ng \u{111}\u{1b0}\u{1ee3}c.".into()
            }
        }
    }

    pub fn smart_dim(self, dim: Dim) -> &'static str {
        match (self, dim) {
            (Language::English, Dim::Length) => "Length",
            (Language::Vietnamese, Dim::Length) => "Chi\u{1ec1}u d\u{e0}i",
            (Language::English, Dim::Mass) => "Weight",
            (Language::Vietnamese, Dim::Mass) => "Kh\u{1ed1}i l\u{1b0}\u{1ee3}ng",
            (Language::English, Dim::Volume) => "Volume",
            (Language::Vietnamese, Dim::Volume) => "Th\u{1ec3} t\u{ed}ch",
            (Language::English, Dim::Area) => "Area",
            (Language::Vietnamese, Dim::Area) => "Di\u{1ec7}n t\u{ed}ch",
            (Language::English, Dim::Speed) => "Speed",
            (Language::Vietnamese, Dim::Speed) => "T\u{1ed1}c \u{111}\u{1ed9}",
            (Language::English, Dim::Temperature) => "Temperature",
            (Language::Vietnamese, Dim::Temperature) => "Nhi\u{1ec7}t \u{111}\u{1ed9}",
            (Language::English, Dim::Data) => "Data size",
            (Language::Vietnamese, Dim::Data) => "Dung l\u{1b0}\u{1ee3}ng",
        }
    }

    pub fn smart_local_time(self) -> &'static str {
        match self {
            Language::English => "Local time",
            Language::Vietnamese => "Gi\u{1edd} \u{111}\u{1ecb}a ph\u{1b0}\u{1a1}ng",
        }
    }

    /// "tomorrow", "yesterday", "+2 days": how far from the day in the text a converted time falls.
    pub fn smart_day(self, delta: i32) -> String {
        match (self, delta) {
            (Language::English, 1) => "tomorrow".into(),
            (Language::English, -1) => "yesterday".into(),
            (Language::English, n) => format!("{n:+} days"),
            (Language::Vietnamese, 1) => "ng\u{e0}y mai".into(),
            (Language::Vietnamese, -1) => "h\u{f4}m qua".into(),
            (Language::Vietnamese, n) => format!("{n:+} ng\u{e0}y"),
        }
    }

    /// A date for the footer: "15 Oct" or "15/10".
    pub fn smart_short_date(self, month: u32, day: u32) -> String {
        const MONTHS: [&str; 12] = [
            "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
        ];
        match self {
            Language::English => format!(
                "{day} {}",
                MONTHS.get(month.saturating_sub(1) as usize).unwrap_or(&"?")
            ),
            Language::Vietnamese => format!("{day}/{month}"),
        }
    }
}

/// Why a translation did not come.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TranslationProblem {
    /// No connection or no answer in time.
    Offline,
    Service(Failure),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_string_exists_in_both_languages() {
        for language in Language::ALL {
            let fixed = [
                language.smart_menu(),
                language.smart_footer(),
                language.smart_footer_message(),
                language.smart_nothing_selected(),
                language.smart_needs_accessibility(),
                language.smart_clipboard_not_restorable(),
                language.smart_internet_for_translation(),
                language.smart_internet_for_rates(),
                language.smart_fetching_rates(),
                language.smart_rates_unavailable(),
                language.smart_nothing_to_show(),
                language.smart_nothing_recognised(),
                language.smart_translating(),
                language.smart_local_time(),
            ];
            for text in fixed {
                assert!(!text.trim().is_empty(), "{language:?}");
            }
            for dim in [
                Dim::Length,
                Dim::Mass,
                Dim::Volume,
                Dim::Area,
                Dim::Speed,
                Dim::Temperature,
                Dim::Data,
            ] {
                assert!(!language.smart_dim(dim).is_empty());
            }
            for problem in [
                TranslationProblem::Offline,
                TranslationProblem::Service(Failure::Quota),
                TranslationProblem::Service(Failure::SameLanguage),
                TranslationProblem::Service(Failure::Service(500)),
                TranslationProblem::Service(Failure::Unreadable),
            ] {
                assert!(!language.smart_translation_failed(&problem, "X").is_empty());
            }
            assert!(language.smart_translate_header("X").contains('X'));
            assert!(language.smart_too_long(450).contains("450"));
            assert!(language.smart_footer_rates("15 Oct").contains("15 Oct"));
        }
        // Every string is in both languages, not the same text twice.
        assert_ne!(
            Language::English.smart_translating(),
            Language::Vietnamese.smart_translating()
        );
    }

    #[test]
    fn days_and_dates_are_worded_for_each_language() {
        assert_eq!(Language::English.smart_day(1), "tomorrow");
        assert_eq!(Language::English.smart_day(-1), "yesterday");
        assert_eq!(Language::English.smart_day(2), "+2 days");
        assert_eq!(Language::English.smart_day(-3), "-3 days");
        assert_eq!(Language::Vietnamese.smart_day(1), "ng\u{e0}y mai");
        assert_eq!(Language::English.smart_short_date(10, 15), "15 Oct");
        assert_eq!(Language::Vietnamese.smart_short_date(10, 15), "15/10");
        assert_eq!(
            Language::English.smart_short_date(13, 1),
            "1 ?",
            "a bad month is not a crash"
        );
    }
}
