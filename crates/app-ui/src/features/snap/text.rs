//! The words this feature puts on screen outside the settings page: the name under each layout of the row, and the
//! labels read by assistive technology.

use crate::i18n::Language;

impl Language {
    /// The name of a built-in layout, short enough for the width of a tile under its small screen; the drawing says
    /// the rest. Empty for an id that is not one.
    pub fn snap_template(self, id: &str) -> &'static str {
        match (self, id) {
            (Language::English, "full") => "Full screen",
            (Language::English, "halves") => "Halves",
            (Language::English, "rows") => "Rows",
            (Language::English, "thirds") => "Thirds",
            (Language::English, "quarters") => "Quarters",
            (Language::English, "main_left") => "Wide left",
            (Language::English, "main_right") => "Wide right",
            (Language::English, "left_stack") => "1 + 2",
            (Language::English, "right_stack") => "2 + 1",
            (Language::English, "grid_six") => "Six",
            (Language::Vietnamese, "full") => "Tối đa",
            (Language::Vietnamese, "halves") => "Chia đôi",
            (Language::Vietnamese, "rows") => "Hai hàng",
            (Language::Vietnamese, "thirds") => "Chia ba",
            (Language::Vietnamese, "quarters") => "Bốn góc",
            (Language::Vietnamese, "main_left") => "Rộng trái",
            (Language::Vietnamese, "main_right") => "Rộng phải",
            (Language::Vietnamese, "left_stack") => "1 + 2",
            (Language::Vietnamese, "right_stack") => "2 + 1",
            (Language::Vietnamese, "grid_six") => "Sáu ô",
            _ => "",
        }
    }

    /// The label of the whole row: what it is, then each layout in order.
    pub fn snap_row(self, names: &[String]) -> String {
        let lead = match self {
            Language::English => "Snap layouts",
            Language::Vietnamese => "Bố cục ghim cửa sổ",
        };
        format!("{lead}: {}", names.join(", "))
    }

    /// The label of the chip the row shrinks to.
    pub fn snap_chosen(self, name: &str) -> String {
        match self {
            Language::English => format!("Chosen layout: {name}"),
            Language::Vietnamese => format!("Bố cục đã chọn: {name}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_window::templates::BUILT_IN;

    #[test]
    fn every_built_in_layout_has_a_short_name_in_both_languages() {
        for language in Language::ALL {
            for id in BUILT_IN {
                let name = language.snap_template(id);
                // About what fits on one line under a tile.
                assert!(
                    !name.trim().is_empty() && name.chars().count() <= 12,
                    "{id} in {language:?}: {name:?}"
                );
            }
            assert_eq!(language.snap_template("bogus"), "");
            let row = language.snap_row(&["A".into(), "B".into()]);
            assert!(row.ends_with(": A, B"), "{row}");
            assert!(language.snap_chosen("Thirds").ends_with("Thirds"));
        }
        assert_ne!(
            Language::English.snap_template("halves"),
            Language::Vietnamese.snap_template("halves")
        );
    }
}
