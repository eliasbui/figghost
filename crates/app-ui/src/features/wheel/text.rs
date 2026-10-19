//! The words the drag wheel puts on screen outside the settings page: the titles of its tools and actions, and
//! the caption in the hub. Format names (PNG, ZIP, M4A) are written the same in every language and are not here.

use super::catalog::Kind;
use crate::i18n::Language;

impl Language {
    /// The title of a tool or an action slot, at most 12 characters so that it fits in a slot. `None` for the
    /// ids that name a format.
    pub fn wheel_title(self, id: &str) -> Option<&'static str> {
        let (english, vietnamese) = match id {
            "compress" => ("Compress", "Nén"),
            "compress_1080p" => ("Max 1080p", "Tối đa 1080p"),
            "compress_720p" => ("Max 720p", "Tối đa 720p"),
            "compress_540p" => ("Max 540p", "Tối đa 540p"),
            "half" => ("Half size", "Một nửa"),
            "quarter" => ("Quarter size", "Một phần tư"),
            "strip_metadata" => ("No metadata", "Xóa metadata"),
            "remove_background" => ("Cut out", "Tách nền"),
            "copy_text" => ("Copy text", "Chép chữ"),
            "annotate" => ("Annotate", "Chú thích"),
            "pdf_merge" => ("Merge PDFs", "Gộp PDF"),
            "pdf_split" => ("Split pages", "Tách trang"),
            "copy_path" => ("Copy Path", "Đường dẫn"),
            "copy" => ("Copy", "Sao chép"),
            "airdrop" => ("AirDrop", "AirDrop"),
            "terminal" => ("Terminal", "Terminal"),
            "reveal" => ("Reveal", "Trong Finder"),
            "float" => ("Float", "Lên khay"),
            _ => return None,
        };
        Some(match self {
            Language::English => english,
            Language::Vietnamese => vietnamese,
        })
    }

    /// Shown in the hub and read by assistive technology: what the wheel does with how many files.
    pub fn wheel_caption(self, kind: Kind, count: usize) -> String {
        match (self, kind) {
            (Language::English, Kind::Formats) => format!("Convert {}", english_files(count)),
            (Language::English, Kind::Tools) => format!("Tools for {}", english_files(count)),
            (Language::English, Kind::Actions) => format!("Actions for {}", english_files(count)),
            (Language::Vietnamese, Kind::Formats) => format!("Chuyển đổi {count} tệp"),
            (Language::Vietnamese, Kind::Tools) => format!("Công cụ cho {count} tệp"),
            (Language::Vietnamese, Kind::Actions) => format!("Thao tác cho {count} tệp"),
        }
    }

    /// The toast after files were made next to the originals.
    pub fn wheel_made(self, count: usize) -> String {
        match self {
            Language::English => format!("Created {}", english_files(count)),
            Language::Vietnamese => format!("Đã tạo {count} tệp"),
        }
    }

    /// The toast when some files were made and then one job failed.
    pub fn wheel_partial(self, made: usize, detail: &str) -> String {
        match self {
            Language::English => format!("Created {made}, then it failed: {detail}"),
            Language::Vietnamese => format!("Đã tạo {made} tệp, rồi bị lỗi: {detail}"),
        }
    }

    pub fn wheel_failed(self, detail: &str) -> String {
        match self {
            Language::English => format!("Could not finish: {detail}"),
            Language::Vietnamese => format!("Không hoàn tất được: {detail}"),
        }
    }

    /// The toast after the paths of the files went to the clipboard.
    pub fn wheel_copied_paths(self, count: usize) -> String {
        match self {
            Language::English if count == 1 => "Copied 1 path".to_owned(),
            Language::English => format!("Copied {count} paths"),
            Language::Vietnamese => format!("Đã sao chép {count} đường dẫn"),
        }
    }

    /// The toast after the files themselves went to the clipboard.
    pub fn wheel_copied_files(self, count: usize) -> String {
        match self {
            Language::English => format!("Copied {}", english_files(count)),
            Language::Vietnamese => format!("Đã sao chép {count} tệp"),
        }
    }

    pub fn wheel_floated(self, count: usize) -> String {
        match self {
            Language::English => format!("Added {} to the shelf", english_files(count)),
            Language::Vietnamese => format!("Đã thêm {count} tệp vào khay"),
        }
    }

    pub fn wheel_no_shelf(self) -> &'static str {
        match self {
            Language::English => "The shelf needs the notch",
            Language::Vietnamese => "Khay cần có notch",
        }
    }
}

fn english_files(count: usize) -> String {
    if count == 1 {
        "1 file".to_owned()
    } else {
        format!("{count} files")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tool_and_action_has_a_short_title_in_both_languages() {
        for id in super::super::catalog::NAMED_IDS {
            for language in Language::ALL {
                let title = language.wheel_title(id).unwrap_or_else(|| panic!("{id}"));
                assert!(!title.trim().is_empty(), "{id} in {language:?}");
                // About what fits on one line in a slot.
                assert!(title.chars().count() <= 12, "{title:?}");
            }
        }
        assert_ne!(
            Language::English.wheel_title("compress"),
            Language::Vietnamese.wheel_title("compress")
        );
        assert_eq!(Language::English.wheel_title("to_png"), None);
    }

    #[test]
    fn the_notices_say_how_many_files_in_both_languages() {
        for language in Language::ALL {
            for text in [
                language.wheel_made(3),
                language.wheel_partial(2, "boom"),
                language.wheel_failed("boom"),
                language.wheel_copied_paths(2),
                language.wheel_copied_files(2),
                language.wheel_floated(2),
                language.wheel_no_shelf().to_owned(),
            ] {
                assert!(!text.trim().is_empty(), "{language:?}");
            }
        }
        assert_eq!(Language::English.wheel_made(1), "Created 1 file");
        assert_eq!(Language::English.wheel_made(3), "Created 3 files");
        assert_eq!(Language::Vietnamese.wheel_made(3), "Đã tạo 3 tệp");
        assert!(Language::English.wheel_partial(2, "boom").contains("boom"));
    }

    #[test]
    fn the_caption_counts_the_files() {
        assert_eq!(
            Language::English.wheel_caption(Kind::Formats, 3),
            "Convert 3 files"
        );
        assert_eq!(
            Language::English.wheel_caption(Kind::Tools, 1),
            "Tools for 1 file"
        );
        assert_eq!(
            Language::Vietnamese.wheel_caption(Kind::Actions, 2),
            "Thao tác cho 2 tệp"
        );
    }
}
