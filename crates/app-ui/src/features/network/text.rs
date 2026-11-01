//! The words of what the Network feature says on the notch, in each language.

use crate::i18n::Language;

fn pick(language: Language, en: String, vi: String) -> String {
    match language {
        Language::English => en,
        Language::Vietnamese => vi,
    }
}

/// Bytes the way a person reads them: `850 MB`, `1.2 GB`.
pub fn bytes(n: u64) -> String {
    let n = n as f64;
    if n >= 1e12 {
        format!("{:.1} TB", n / 1e12)
    } else if n >= 1e9 {
        format!("{:.1} GB", n / 1e9)
    } else if n >= 1e6 {
        format!("{:.0} MB", n / 1e6)
    } else {
        format!("{:.0} KB", n / 1e3)
    }
}

/// What an app has used of its quota: the title and the line under it.
pub fn quota(language: Language, app: &str, used: u64, quota: u64, over: bool) -> (String, String) {
    let line = format!("{} / {}", bytes(used), bytes(quota));
    let title = if over {
        pick(
            language,
            format!("{app} reached its data quota"),
            format!("{app} đã dùng hết hạn mức dữ liệu"),
        )
    } else {
        pick(
            language,
            format!("{app} is close to its data quota"),
            format!("{app} sắp hết hạn mức dữ liệu"),
        )
    };
    (title, line)
}

/// What the metered connections have used of their cap.
pub fn cap(language: Language, used: u64, cap: u64, over: bool) -> (String, String) {
    let line = format!("{} / {}", bytes(used), bytes(cap));
    let title = if over {
        pick(
            language,
            "Data cap reached: apps are blocked".to_owned(),
            "Đã hết hạn mức: các app bị chặn".to_owned(),
        )
    } else {
        pick(
            language,
            "Close to the data cap of this connection".to_owned(),
            "Sắp hết hạn mức dữ liệu của đường mạng này".to_owned(),
        )
    };
    (title, line)
}

/// The internet went away.
pub fn offline(language: Language) -> (String, String) {
    (
        pick(
            language,
            "No internet".to_owned(),
            "Mất kết nối internet".to_owned(),
        ),
        pick(
            language,
            "The connection is up but nothing gets through".to_owned(),
            "Đã nối mạng nhưng không ra được internet".to_owned(),
        ),
    )
}

/// The internet came back.
pub fn back_online(language: Language) -> (String, String) {
    (
        pick(
            language,
            "Back online".to_owned(),
            "Đã có internet trở lại".to_owned(),
        ),
        String::new(),
    )
}

/// The network asks for a login.
pub fn needs_login(language: Language) -> (String, String) {
    (
        pick(
            language,
            "This network asks you to sign in".to_owned(),
            "Mạng này yêu cầu đăng nhập".to_owned(),
        ),
        pick(
            language,
            "Open any website to see its login page".to_owned(),
            "Mở một trang web bất kỳ để thấy trang đăng nhập".to_owned(),
        ),
    )
}

/// An app that sends far more than it usually does.
pub fn upload(language: Language, app: &str, sent: u64) -> (String, String) {
    (
        pick(
            language,
            format!("{app} is uploading a lot"),
            format!("{app} đang tải lên rất nhiều"),
        ),
        pick(
            language,
            format!("{} in the last 5 minutes", bytes(sent)),
            format!("{} trong 5 phút qua", bytes(sent)),
        ),
    )
}

/// The Mac moved onto a metered connection and only the allowed apps go on.
pub fn metered_on(language: Language, link: &str) -> (String, String) {
    (
        pick(
            language,
            "Metered connection: saving data".to_owned(),
            "Đường mạng có giới hạn: đang tiết kiệm data".to_owned(),
        ),
        link.to_owned(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytes_are_said_in_the_unit_that_fits() {
        assert_eq!(bytes(0), "0 KB");
        assert_eq!(bytes(850_000_000), "850 MB");
        assert_eq!(bytes(1_240_000_000), "1.2 GB");
        assert_eq!(bytes(2_000_000_000_000), "2.0 TB");
    }

    #[test]
    fn the_notices_name_the_app_and_say_it_in_both_languages() {
        let (en, line) = quota(Language::English, "Steam", 90_000_000, 100_000_000, false);
        assert!(en.contains("Steam") && en.contains("close"));
        assert_eq!(line, "90 MB / 100 MB");
        let (vi, _) = quota(
            Language::Vietnamese,
            "Steam",
            100_000_000,
            100_000_000,
            true,
        );
        assert!(vi.contains("Steam") && vi.contains("hết"));
        assert_ne!(
            cap(Language::English, 1, 2, true).0,
            cap(Language::Vietnamese, 1, 2, true).0
        );
        assert_ne!(
            cap(Language::English, 1, 2, true).0,
            cap(Language::English, 1, 2, false).0
        );
        let (title, line) = upload(Language::Vietnamese, "Dropbox", 120_000_000);
        assert!(title.contains("Dropbox") && line.starts_with("120 MB"));
        assert!(
            upload(Language::English, "Dropbox", 1)
                .1
                .contains("5 minutes")
        );
        for words in [offline, back_online, needs_login] {
            assert_ne!(words(Language::English).0, words(Language::Vietnamese).0);
        }
        assert_ne!(
            metered_on(Language::English, "en1").0,
            metered_on(Language::Vietnamese, "en1").0
        );
    }
}
