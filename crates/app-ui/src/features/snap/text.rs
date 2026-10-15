//! The words this feature puts on screen outside the settings page: the name of each zone, read by assistive
//! technology, since the row itself draws shapes only.

use crate::i18n::Language;
use core_window::Layout;

impl Language {
    pub fn snap_zone(self, layout: Layout) -> &'static str {
        match (self, layout) {
            (Language::English, Layout::Left) => "Left half",
            (Language::English, Layout::Right) => "Right half",
            (Language::English, Layout::Top) => "Top half",
            (Language::English, Layout::Bottom) => "Bottom half",
            (Language::English, Layout::TopLeft) => "Top left",
            (Language::English, Layout::TopRight) => "Top right",
            (Language::English, Layout::BottomLeft) => "Bottom left",
            (Language::English, Layout::BottomRight) => "Bottom right",
            (Language::English, Layout::Maximize) => "Maximize",
            (Language::English, Layout::Center) => "Center",
            (Language::English, Layout::FirstThird) => "First third",
            (Language::English, Layout::CenterThird) => "Center third",
            (Language::English, Layout::LastThird) => "Last third",
            (Language::Vietnamese, Layout::Left) => "Nửa trái",
            (Language::Vietnamese, Layout::Right) => "Nửa phải",
            (Language::Vietnamese, Layout::Top) => "Nửa trên",
            (Language::Vietnamese, Layout::Bottom) => "Nửa dưới",
            (Language::Vietnamese, Layout::TopLeft) => "Góc trên trái",
            (Language::Vietnamese, Layout::TopRight) => "Góc trên phải",
            (Language::Vietnamese, Layout::BottomLeft) => "Góc dưới trái",
            (Language::Vietnamese, Layout::BottomRight) => "Góc dưới phải",
            (Language::Vietnamese, Layout::Maximize) => "Phóng to tối đa",
            (Language::Vietnamese, Layout::Center) => "Giữa màn hình",
            (Language::Vietnamese, Layout::FirstThird) => "Một phần ba đầu",
            (Language::Vietnamese, Layout::CenterThird) => "Một phần ba giữa",
            (Language::Vietnamese, Layout::LastThird) => "Một phần ba cuối",
        }
    }

    /// The label of the whole row: what it is, then each zone in order.
    pub fn snap_zones(self, layouts: &[Layout]) -> String {
        let names: Vec<&str> = layouts.iter().map(|l| self.snap_zone(*l)).collect();
        let lead = match self {
            Language::English => "Snap zones",
            Language::Vietnamese => "Vùng ghim cửa sổ",
        };
        format!("{lead}: {}", names.join(", "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_zone_has_a_name_in_both_languages() {
        for language in Language::ALL {
            for layout in Layout::ALL {
                assert!(
                    !language.snap_zone(layout).trim().is_empty(),
                    "{layout:?} in {language:?}"
                );
            }
            let row = language.snap_zones(&core_window::snap::DEFAULT_ZONES);
            assert!(row.contains(": ") && row.matches(", ").count() == 5);
        }
        assert_ne!(
            Language::English.snap_zone(Layout::Left),
            Language::Vietnamese.snap_zone(Layout::Left)
        );
    }
}
