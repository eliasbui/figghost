//! The words of breaks: the card of a break, the notices on the notch, and the menu.

use crate::i18n::Language;

impl Language {
    pub fn breaks_title(self) -> &'static str {
        match self {
            Language::English => "Look into the distance",
            Language::Vietnamese => "Nhìn ra xa",
        }
    }

    pub fn breaks_hint(self) -> &'static str {
        match self {
            Language::English => {
                "Rest your eyes on something about six metres away, and blink a few times."
            }
            Language::Vietnamese => "Nhìn một vật cách bạn chừng sáu mét, và chớp mắt vài lần.",
        }
    }

    pub fn breaks_skip(self) -> &'static str {
        match self {
            Language::English => "Skip",
            Language::Vietnamese => "Bỏ qua",
        }
    }

    pub fn breaks_later(self) -> &'static str {
        match self {
            Language::English => "In 5 minutes",
            Language::Vietnamese => "Để 5 phút nữa",
        }
    }

    pub fn breaks_soon(self) -> &'static str {
        match self {
            Language::English => "A break for your eyes in 10 seconds",
            Language::Vietnamese => "10 giây nữa nghỉ mắt",
        }
    }

    pub fn breaks_done(self) -> &'static str {
        match self {
            Language::English => "Break over",
            Language::Vietnamese => "Hết giờ nghỉ",
        }
    }

    pub fn breaks_put_off(self) -> &'static str {
        match self {
            Language::English => "The break comes in 5 minutes",
            Language::Vietnamese => "5 phút nữa sẽ nghỉ",
        }
    }

    pub fn breaks_blink(self) -> &'static str {
        match self {
            Language::English => "Blink a few times",
            Language::Vietnamese => "Chớp mắt vài lần",
        }
    }

    pub fn breaks_posture(self) -> &'static str {
        match self {
            Language::English => "Sit up and let your shoulders drop",
            Language::Vietnamese => "Ngồi thẳng lưng, thả lỏng vai",
        }
    }

    pub fn breaks_menu_now(self) -> &'static str {
        match self {
            Language::English => "Rest my eyes now",
            Language::Vietnamese => "Nghỉ mắt ngay",
        }
    }

    pub fn breaks_menu_end(self) -> &'static str {
        match self {
            Language::English => "End the break",
            Language::Vietnamese => "Kết thúc giờ nghỉ",
        }
    }
}
