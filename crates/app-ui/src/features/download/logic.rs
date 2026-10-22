//! What does not need a screen or a process: which text is a link that may be handed to yt-dlp, where the tools are,
//! the arguments for a given choice, and how the lines yt-dlp prints are read. Everything here is plain data in and out,
//! so the tests cover it without running anything.

use crate::features::is_hidden_char;
use std::{
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};

/// The longest link that is accepted.
pub const MAX_URL: usize = 2048;
/// The longest error line kept, in characters.
const MAX_ERROR: usize = 200;
/// The longest title kept, in characters.
const MAX_TITLE: usize = 120;

/// The marks of the lines this feature asks yt-dlp to print, so that they cannot be mistaken for anything else it says.
const PROGRESS_MARK: &str = "FGDL|";
const TITLE_MARK: &str = "FGINFO|";
const FILE_MARK: &str = "FGFILE|";

/// The template that gives yt-dlp's progress as numbers this file can read, whatever the version words it in.
const PROGRESS_TEMPLATE: &str = "download:FGDL|%(progress.status)s|%(progress.downloaded_bytes)s|%(progress.total_bytes)s|%(progress.total_bytes_estimate)s|%(progress.speed)s|%(progress.eta)s";
/// The name of a file: the title cut to a length that no file system refuses, and the id of the video, so that two
/// videos of one title never take the same name.
const OUTPUT_TEMPLATE: &str = "%(title).200B [%(id)s].%(ext)s";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quality {
    Best,
    P1080,
    P720,
    Audio,
}

impl Quality {
    pub const ALL: [Quality; 4] = [Quality::Best, Quality::P1080, Quality::P720, Quality::Audio];

    pub fn key(self) -> &'static str {
        match self {
            Quality::Best => "best",
            Quality::P1080 => "1080",
            Quality::P720 => "720",
            Quality::Audio => "audio",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|q| q.key() == text)
    }

    /// The tallest picture wanted, if there is a limit.
    fn height(self) -> Option<u32> {
        match self {
            Quality::P1080 => Some(1080),
            Quality::P720 => Some(720),
            Quality::Best | Quality::Audio => None,
        }
    }
}

/// The link in `text`, if it is one that may be handed to yt-dlp: http or https, at most `MAX_URL` characters, a host,
/// and no space or control character inside. Whitespace around it, as a copy brings, is not part of it.
pub fn valid_url(text: &str) -> Option<&str> {
    let url = text.trim();
    if url.is_empty() || url.chars().count() > MAX_URL {
        return None;
    }
    if url.chars().any(|c| c.is_whitespace() || is_hidden_char(c)) {
        return None;
    }
    let lower = url.to_ascii_lowercase();
    let rest = lower
        .strip_prefix("https://")
        .or_else(|| lower.strip_prefix("http://"))?;
    // Something must stand where the host goes.
    rest.chars()
        .next()
        .is_some_and(|c| !matches!(c, '/' | '?' | '#'))
        .then_some(url)
}

/// The folder downloads go to: `folder` when it is an absolute path of a folder that exists, else Downloads.
pub fn resolve_folder(folder: &str, home: &Path) -> PathBuf {
    let folder = folder.trim();
    let wanted = Path::new(folder);
    if wanted.is_absolute() && wanted.is_dir() {
        wanted.to_path_buf()
    } else {
        home.join("Downloads")
    }
}

/// The directories a tool is looked for in: the places Homebrew and pip put it, since an app started from the Finder has
/// no search path worth the name, then the search path itself.
pub fn tool_dirs(home: &Path, path_var: &str) -> Vec<PathBuf> {
    let mut dirs = vec![
        PathBuf::from("/opt/homebrew/bin"),
        PathBuf::from("/usr/local/bin"),
        home.join(".local/bin"),
    ];
    dirs.extend(
        path_var
            .split(':')
            .filter(|d| Path::new(d).is_absolute())
            .map(PathBuf::from),
    );
    dirs
}

/// The first executable file called `name` in `dirs`.
pub fn find_tool(name: &str, dirs: &[PathBuf]) -> Option<PathBuf> {
    dirs.iter().map(|d| d.join(name)).find(|p| {
        p.metadata()
            .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
    })
}

/// Everything that decides the arguments of one download.
pub struct Plan<'a> {
    pub url: &'a str,
    pub folder: &'a Path,
    /// Where yt-dlp keeps what it has not finished, so that no half-written file shows in the folder.
    pub temp: &'a Path,
    pub quality: Quality,
    pub playlist: bool,
    pub ffmpeg: Option<&'a Path>,
}

/// The arguments for yt-dlp. The link is the last one and stands after `--`, so that it can never be read as an option;
/// nothing is run through a shell.
pub fn build_args(plan: &Plan) -> Vec<String> {
    let mut args: Vec<String> = [
        // Without this, the person's own configuration may change the format of every line read below.
        "--ignore-config",
        "--color",
        "no_color",
        "--newline",
        "--progress",
        "--progress-template",
        PROGRESS_TEMPLATE,
        "--print",
        "before_dl:FGINFO|%(title)s",
        "--print",
        "after_move:FGFILE|%(filepath)s",
        if plan.playlist {
            "--yes-playlist"
        } else {
            "--no-playlist"
        },
        "-o",
        OUTPUT_TEMPLATE,
    ]
    .map(str::to_owned)
    .into();
    args.push("-P".into());
    args.push(format!("home:{}", plan.folder.display()));
    args.push("-P".into());
    args.push(format!("temp:{}", plan.temp.display()));
    args.push("-f".into());
    args.push(format_selector(plan.quality, plan.ffmpeg.is_some()));
    match (plan.quality, plan.ffmpeg) {
        (Quality::Audio, Some(_)) => {
            args.extend(["-x", "--audio-format", "m4a"].map(str::to_owned));
        }
        (_, Some(_)) => args.extend(["--merge-output-format", "mp4"].map(str::to_owned)),
        _ => {}
    }
    if let Some(ffmpeg) = plan.ffmpeg {
        args.push("--ffmpeg-location".into());
        args.push(ffmpeg.display().to_string());
    }
    args.push("--".into());
    args.push(plan.url.to_owned());
    args
}

/// The `-f` of yt-dlp. With ffmpeg the best video and the best audio are fetched apart and joined; without it only a
/// file that already has both can be used, so the choice is the best of those, an mp4 if there is one.
fn format_selector(quality: Quality, ffmpeg: bool) -> String {
    let cap = quality
        .height()
        .map(|h| format!("[height<=?{h}]"))
        .unwrap_or_default();
    match (quality, ffmpeg) {
        (Quality::Audio, true) => "ba/b".into(),
        (Quality::Audio, false) => "ba[ext=m4a]/ba/b".into(),
        (_, true) => format!("bv*{cap}+ba/b{cap}"),
        (_, false) => format!("b{cap}[ext=mp4]/b{cap}"),
    }
}

/// What one progress line says. A number the line does not have is `None`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Progress {
    pub percent: Option<f64>,
    /// Bytes a second.
    pub speed: Option<f64>,
    pub eta_seconds: Option<u64>,
    pub finished: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Line {
    Title(String),
    Progress(Progress),
    /// The path of a finished file.
    File(PathBuf),
    /// The text of an error, cut short.
    Error(String),
    Other,
}

fn number(field: &str) -> Option<f64> {
    field.parse::<f64>().ok().filter(|n| n.is_finite())
}

/// Reads one line of what yt-dlp printed, from its output or from its error stream.
pub fn parse_line(line: &str) -> Line {
    let line = line.trim_end();
    if let Some(rest) = line.strip_prefix(PROGRESS_MARK) {
        return parse_progress(rest).map_or(Line::Other, Line::Progress);
    }
    if let Some(title) = line.strip_prefix(TITLE_MARK) {
        return Line::Title(clean(title, MAX_TITLE));
    }
    if let Some(path) = line.strip_prefix(FILE_MARK) {
        let path = Path::new(path);
        return if path.is_absolute() {
            Line::File(path.to_path_buf())
        } else {
            Line::Other
        };
    }
    if let Some(error) = line.strip_prefix("ERROR:") {
        let error = error.trim();
        if !error.is_empty() {
            return Line::Error(clean(error, MAX_ERROR));
        }
    }
    Line::Other
}

/// `status|downloaded|total|estimate|speed|eta`, with `NA` for what is not known.
fn parse_progress(rest: &str) -> Option<Progress> {
    let fields: Vec<&str> = rest.split('|').collect();
    let [status, downloaded, total, estimate, speed, eta] = fields[..] else {
        return None;
    };
    // The exact size is used when the site gives one, else its estimate.
    let total = number(total).or_else(|| number(estimate));
    let downloaded = number(downloaded);
    let speed = number(speed).filter(|s| *s >= 0.0);
    let eta = number(eta).filter(|s| *s >= 0.0);
    let percent = match (downloaded, total) {
        (Some(done), Some(all)) if all > 0.0 => Some((done / all * 100.0).clamp(0.0, 100.0)),
        _ => None,
    };
    Some(Progress {
        percent,
        speed,
        eta_seconds: eta.map(|s| s as u64),
        finished: status == "finished",
    })
}

/// Text for the person to read: no hidden character, no more than `max` characters.
fn clean(text: &str, max: usize) -> String {
    let shown: String = text.chars().filter(|c| !is_hidden_char(*c)).collect();
    let shown = shown.trim();
    if shown.chars().count() <= max {
        return shown.to_owned();
    }
    let mut cut: String = shown.chars().take(max.saturating_sub(3)).collect();
    cut.push_str("...");
    cut
}

/// Whether an error says that the video cannot be had without ffmpeg: the sites that only offer a picture and a sound
/// apart leave no single file to choose.
pub fn needs_ffmpeg(error: &str, ffmpeg: bool) -> bool {
    let lower = error.to_lowercase();
    lower.contains("ffmpeg") || (!ffmpeg && lower.contains("requested format is not available"))
}

/// The host of a link, for a download whose title is not known yet.
pub fn host(url: &str) -> String {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    let host = rest.split(['/', '?', '#']).next().unwrap_or_default();
    clean(host.strip_prefix("www.").unwrap_or(host), 14)
}

/// The short name shown beside the percentage on the notch: the title cut to what the closed notch has room for.
pub fn short_name(title: &str) -> String {
    clean(title, 14)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_link_is_http_or_https_with_a_host_and_nothing_else_in_it() {
        for good in [
            "https://www.youtube.com/watch?v=BaW_jenozKc",
            "http://example.com/a.mp4",
            "HTTPS://Vimeo.com/123",
            "  https://example.com/x  \n",
            "https://example.com/a%20b?c=d#e",
        ] {
            assert_eq!(valid_url(good), Some(good.trim()), "{good:?}");
        }
        let long = format!("https://example.com/{}", "a".repeat(MAX_URL));
        for bad in [
            "",
            "   ",
            "example.com/x",
            "ftp://example.com/x",
            "file:///etc/passwd",
            "javascript:alert(1)",
            "https://",
            "https:///x",
            "https://?x",
            "-o /tmp/x https://example.com",
            "https://example.com/a b",
            "https://example.com/a\tb",
            "https://example.com/a\nb",
            "https://example.com/a\u{7}b",
            "https://example.com/a\u{202E}b",
            "https://example.com --exec id",
            long.as_str(),
        ] {
            assert_eq!(valid_url(bad), None, "{bad:?}");
        }
        let exact = format!("https://e.com/{}", "a".repeat(MAX_URL - 14));
        assert_eq!(exact.chars().count(), MAX_URL);
        assert_eq!(
            valid_url(&exact),
            Some(exact.as_str()),
            "the limit itself is allowed"
        );
    }

    #[test]
    fn the_folder_is_the_one_asked_for_only_when_it_is_an_absolute_folder_that_exists() {
        let home = Path::new("/Users/someone");
        let here = std::env::temp_dir();
        assert_eq!(resolve_folder("", home), home.join("Downloads"));
        assert_eq!(
            resolve_folder(here.to_str().unwrap(), home),
            here,
            "a real folder is used"
        );
        for bad in ["relative/dir", "~/Movies", "/does/not/exist/at/all", "  "] {
            assert_eq!(resolve_folder(bad, home), home.join("Downloads"), "{bad}");
        }
        let file = here.join(format!("figghost-dl-notdir-{}", std::process::id()));
        std::fs::write(&file, "x").unwrap();
        assert_eq!(
            resolve_folder(file.to_str().unwrap(), home),
            home.join("Downloads"),
            "a file is not a folder"
        );
        std::fs::remove_file(&file).unwrap();
    }

    fn plan<'a>(quality: Quality, playlist: bool, ffmpeg: Option<&'a Path>) -> Plan<'a> {
        Plan {
            url: "https://example.com/v",
            folder: Path::new("/Users/x/Downloads"),
            temp: Path::new("/Users/x/Downloads/.figghost-download-1"),
            quality,
            playlist,
            ffmpeg,
        }
    }

    fn after(args: &[String], flag: &str) -> Option<String> {
        let at = args.iter().position(|a| a == flag)?;
        args.get(at + 1).cloned()
    }

    #[test]
    fn the_link_always_stands_alone_after_the_double_dash_and_the_user_config_is_ignored() {
        let ffmpeg = Path::new("/opt/homebrew/bin/ffmpeg");
        for quality in Quality::ALL {
            for with in [None, Some(ffmpeg)] {
                for playlist in [false, true] {
                    let args = build_args(&plan(quality, playlist, with));
                    let n = args.len();
                    assert_eq!(args[n - 2], "--");
                    assert_eq!(args[n - 1], "https://example.com/v");
                    assert_eq!(args.iter().filter(|a| *a == "--").count(), 1);
                    assert_eq!(args[0], "--ignore-config");
                    assert_eq!(
                        args.iter().any(|a| a == "--no-playlist"),
                        !playlist,
                        "{quality:?} {with:?}"
                    );
                    assert_eq!(args.iter().any(|a| a == "--yes-playlist"), playlist);
                }
            }
        }
    }

    #[test]
    fn the_name_of_a_file_is_cut_and_stays_inside_the_folder_chosen() {
        let args = build_args(&plan(Quality::Best, false, None));
        assert_eq!(after(&args, "-o").as_deref(), Some(OUTPUT_TEMPLATE));
        assert!(OUTPUT_TEMPLATE.contains(".200B"));
        let paths: Vec<&String> = args
            .iter()
            .enumerate()
            .filter(|(i, _)| *i > 0 && args[i - 1] == "-P")
            .map(|(_, a)| a)
            .collect();
        assert_eq!(
            paths,
            [
                "home:/Users/x/Downloads",
                "temp:/Users/x/Downloads/.figghost-download-1"
            ]
        );
    }

    #[test]
    fn the_format_follows_the_quality_and_whether_ffmpeg_is_there() {
        let ffmpeg = Path::new("/opt/homebrew/bin/ffmpeg");
        let format = |q, with| after(&build_args(&plan(q, false, with)), "-f").unwrap();
        assert_eq!(format(Quality::Best, Some(ffmpeg)), "bv*+ba/b");
        assert_eq!(
            format(Quality::P1080, Some(ffmpeg)),
            "bv*[height<=?1080]+ba/b[height<=?1080]"
        );
        assert_eq!(
            format(Quality::P720, Some(ffmpeg)),
            "bv*[height<=?720]+ba/b[height<=?720]"
        );
        assert_eq!(format(Quality::Audio, Some(ffmpeg)), "ba/b");
        assert_eq!(format(Quality::Best, None), "b[ext=mp4]/b");
        assert_eq!(
            format(Quality::P720, None),
            "b[height<=?720][ext=mp4]/b[height<=?720]"
        );
        assert_eq!(format(Quality::Audio, None), "ba[ext=m4a]/ba/b");

        let joined = build_args(&plan(Quality::Best, false, Some(ffmpeg)));
        assert_eq!(
            after(&joined, "--merge-output-format").as_deref(),
            Some("mp4")
        );
        assert_eq!(
            after(&joined, "--ffmpeg-location").as_deref(),
            Some("/opt/homebrew/bin/ffmpeg")
        );
        assert!(!joined.iter().any(|a| a == "-x"));
        let music = build_args(&plan(Quality::Audio, false, Some(ffmpeg)));
        assert_eq!(after(&music, "--audio-format").as_deref(), Some("m4a"));
        assert!(music.iter().any(|a| a == "-x"));
        assert!(!music.iter().any(|a| a == "--merge-output-format"));
        for q in Quality::ALL {
            let alone = build_args(&plan(q, false, None));
            for flag in ["--merge-output-format", "-x", "--ffmpeg-location"] {
                assert!(
                    !alone.iter().any(|a| a == flag),
                    "{q:?} without ffmpeg has {flag}"
                );
            }
        }
    }

    #[test]
    fn a_quality_is_read_by_its_key_and_nothing_else() {
        for q in Quality::ALL {
            assert_eq!(Quality::parse(q.key()), Some(q));
        }
        for bad in ["", "4k", "Best", "1080p", "720 "] {
            assert_eq!(Quality::parse(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn a_progress_line_gives_the_percentage_the_speed_and_the_time_left() {
        let Line::Progress(p) = parse_line("FGDL|downloading|1024|2048|NA|5674739.7|3") else {
            panic!("not a progress line")
        };
        assert_eq!(p.percent, Some(50.0));
        assert_eq!(p.speed, Some(5674739.7));
        assert_eq!(p.eta_seconds, Some(3));
        assert!(!p.finished);
        // The size is only an estimate: the percentage is worked out from it.
        let Line::Progress(p) = parse_line("FGDL|downloading|500|NA|1000|NA|NA") else {
            panic!()
        };
        assert_eq!(
            (p.percent, p.speed, p.eta_seconds),
            (Some(50.0), None, None)
        );
        // No size at all: no percentage, and no division by zero.
        let Line::Progress(p) = parse_line("FGDL|downloading|500|NA|NA|NA|NA") else {
            panic!()
        };
        assert_eq!(p.percent, None);
        let Line::Progress(p) = parse_line("FGDL|downloading|5|0|NA|NA|NA") else {
            panic!()
        };
        assert_eq!(p.percent, None);
        // More than the size said is shown as complete, not as 120 percent.
        let Line::Progress(p) = parse_line("FGDL|downloading|1200|1000|NA|10|0") else {
            panic!()
        };
        assert_eq!(p.percent, Some(100.0));
        let Line::Progress(p) = parse_line("FGDL|finished|788493|788493|NA|7770390.8|NA") else {
            panic!()
        };
        assert!(p.finished);
        assert_eq!(p.percent, Some(100.0));
        for odd in [
            "FGDL|",
            "FGDL|downloading",
            "FGDL|x|y|z|1|2|3",
            "FGDL|downloading|-1|-5|NA|-3|-4",
        ] {
            // Whatever it makes of it, it neither panics nor invents a negative number.
            if let Line::Progress(p) = parse_line(odd) {
                assert!(p.speed.is_none_or(|s| s >= 0.0), "{odd}");
                assert!(
                    p.percent.is_none_or(|s| (0.0..=100.0).contains(&s)),
                    "{odd}"
                );
            }
        }
    }

    #[test]
    fn the_title_the_file_and_the_error_are_read_and_anything_else_is_left_alone() {
        assert_eq!(
            parse_line("FGINFO|Me at the zoo"),
            Line::Title("Me at the zoo".into())
        );
        assert_eq!(
            parse_line("FGFILE|/Users/x/Downloads/a [b].mp4"),
            Line::File(PathBuf::from("/Users/x/Downloads/a [b].mp4"))
        );
        assert_eq!(
            parse_line("FGFILE|relative/a.mp4"),
            Line::Other,
            "a path must be absolute"
        );
        assert_eq!(parse_line("FGFILE|"), Line::Other);
        assert_eq!(
            parse_line("ERROR: [youtube] abc: This video is unavailable"),
            Line::Error("[youtube] abc: This video is unavailable".into())
        );
        assert_eq!(parse_line("ERROR:"), Line::Other);
        for other in [
            "[download] Destination: x",
            "WARNING: something",
            "",
            "ERRORS: no",
            " FGDL|downloading|1|2|NA|NA|NA",
        ] {
            assert_eq!(parse_line(other), Line::Other, "{other:?}");
        }
        let Line::Error(long) = parse_line(&format!("ERROR: {}", "x".repeat(900))) else {
            panic!()
        };
        assert_eq!(long.chars().count(), MAX_ERROR, "an error is cut short");
        assert!(long.ends_with("..."));
        let Line::Title(shown) = parse_line("FGINFO|a\u{202E}b\u{7}c") else {
            panic!()
        };
        assert_eq!(shown, "abc", "a title cannot reorder or hide what is shown");
        assert_eq!(
            parse_line("FGINFO|trailing space  \r"),
            Line::Title("trailing space".into())
        );
    }

    #[test]
    fn ffmpeg_is_blamed_only_when_the_error_points_at_it() {
        assert!(needs_ffmpeg("ffmpeg not found. Please install", true));
        assert!(needs_ffmpeg("Requested format is not available", false));
        assert!(!needs_ffmpeg("Requested format is not available", true));
        assert!(!needs_ffmpeg("This video is unavailable", false));
    }

    #[test]
    fn the_host_stands_in_for_a_title_that_is_not_known_yet() {
        assert_eq!(host("https://www.youtube.com/watch?v=x"), "youtube.com");
        assert_eq!(host("http://example.com"), "example.com");
        assert_eq!(host("https://example.com?x=1"), "example.com");
        assert_eq!(
            host("https://a.very.long.host.name.example.org/x")
                .chars()
                .count(),
            14
        );
    }

    #[test]
    fn a_short_name_fits_the_closed_notch() {
        assert_eq!(short_name("Short"), "Short");
        let name = short_name("A very long title that goes on and on");
        assert_eq!(name.chars().count(), 14);
        assert!(name.ends_with("..."));
    }

    #[test]
    fn a_tool_is_the_first_executable_file_of_that_name_in_the_directories_given() {
        let root = std::env::temp_dir().join(format!("figghost-dl-tools-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let (first, second, third) = (root.join("a"), root.join("b"), root.join("c"));
        for d in [&first, &second, &third] {
            std::fs::create_dir_all(d).unwrap();
        }
        // A file that cannot be run is not a tool, a folder of that name neither.
        std::fs::write(first.join("yt-dlp"), "x").unwrap();
        std::fs::create_dir(second.join("yt-dlp")).unwrap();
        std::fs::write(third.join("yt-dlp"), "x").unwrap();
        std::fs::set_permissions(third.join("yt-dlp"), std::fs::Permissions::from_mode(0o755))
            .unwrap();
        let dirs = [first.clone(), second.clone(), third.clone()];
        assert_eq!(find_tool("yt-dlp", &dirs), Some(third.join("yt-dlp")));
        assert_eq!(find_tool("ffmpeg", &dirs), None);
        std::fs::remove_dir_all(&root).unwrap();

        let dirs = tool_dirs(Path::new("/Users/x"), "/usr/bin:relative:/bin");
        assert_eq!(dirs[0], Path::new("/opt/homebrew/bin"));
        assert_eq!(dirs[1], Path::new("/usr/local/bin"));
        assert_eq!(dirs[2], Path::new("/Users/x/.local/bin"));
        assert_eq!(
            dirs[3..],
            [PathBuf::from("/usr/bin"), PathBuf::from("/bin")]
        );
    }
}
