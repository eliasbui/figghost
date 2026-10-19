//! Puts a release's app in place of the one that is running: download, check, swap, clean up.
//!
//! Everything before the swap happens in a folder beside the installed app (the same disk, which makes the
//! swap a rename), and the installed app is not touched until the new one has passed every check. A failure at
//! any step removes that folder and leaves the installed app as it was. The running program keeps going from
//! the files it already has open; the new version starts with the next launch.

use super::release::{Asset, Release};
use std::{
    ffi::OsStr,
    fs,
    os::unix::fs::DirBuilderExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::Duration,
};

const CURL: &str = "/usr/bin/curl";
const DITTO: &str = "/usr/bin/ditto";
const SHASUM: &str = "/usr/bin/shasum";
const PLUTIL: &str = "/usr/bin/plutil";
const CODESIGN: &str = "/usr/bin/codesign";
const BUNDLE_ID: &str = "vn.figghost.app";
/// The app inside the zip, whatever the installed copy has been renamed to.
const APP: &str = "Figghost.app";
/// Where the new copy is unpacked, and where the old one waits to be deleted: both beside the installed app.
const STAGE: &str = ".Figghost-update";
const OLD: &str = ".Figghost-old";
const MAX_ZIP: u64 = 300 << 20;
const DOWNLOAD_SECONDS: u64 = 600;
/// How often the size of the file that is arriving is looked at, for the percentage.
const PROGRESS_INTERVAL: Duration = Duration::from_millis(250);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// How much of the zip has arrived, in percent.
    Downloading(u8),
    Checking,
    Installing,
}

impl Stage {
    pub fn name(self) -> &'static str {
        match self {
            Stage::Downloading(_) => "downloading",
            Stage::Checking => "checking",
            Stage::Installing => "installing",
        }
    }

    pub fn percent(self) -> Option<u8> {
        match self {
            Stage::Downloading(percent) => Some(percent),
            _ => None,
        }
    }
}

/// The app this program runs from, `Figghost.app` of `Figghost.app/Contents/MacOS/Figghost`, with every link
/// followed so that it is the real folder that gets replaced. None for a program run from a build folder.
pub fn running_bundle() -> Option<PathBuf> {
    bundle_of(&std::env::current_exe().ok()?.canonicalize().ok()?)
}

fn bundle_of(exe: &Path) -> Option<PathBuf> {
    let macos = exe
        .parent()
        .filter(|d| d.file_name() == Some(OsStr::new("MacOS")))?;
    let contents = macos
        .parent()
        .filter(|d| d.file_name() == Some(OsStr::new("Contents")))?;
    let bundle = contents.parent()?;
    bundle
        .extension()
        .is_some_and(|e| e == "app")
        .then(|| bundle.to_path_buf())
}

/// Removes what an update that was cut short left beside the app. The old copy is only deleted while the
/// app itself is there, since after a failed swap it could be the only one.
pub fn tidy(bundle: &Path) {
    let Some(folder) = bundle.parent() else {
        return;
    };
    let _ = fs::remove_dir_all(folder.join(STAGE));
    if bundle.exists() {
        let _ = fs::remove_dir_all(folder.join(OLD));
    }
}

/// Downloads `release`'s app, checks it and puts it where `bundle` is. `progress` hears each stage as it starts.
pub fn install(release: &Release, bundle: &Path, progress: impl Fn(Stage)) -> Result<(), String> {
    // Put there already by other means while this copy was running (Homebrew, the install script): a restart
    // is all that is left.
    if plist_value(bundle, "CFBundleShortVersionString").is_ok_and(|found| found == release.version)
    {
        return Ok(());
    }
    let asset = release
        .asset
        .as_ref()
        .ok_or("this release has no app to download")?;
    let folder = bundle.parent().ok_or("the app has no folder")?;
    tidy(bundle);
    let stage = folder.join(STAGE);
    fs::DirBuilder::new()
        .mode(0o700)
        .create(&stage)
        .map_err(|e| format!("cannot write to {}: {e}", folder.display()))?;
    let old = folder.join(OLD);
    let done = prepare(release, asset, &stage, &progress).and_then(|fresh| {
        progress(Stage::Installing);
        swap(bundle, &fresh, &old)
    });
    if done.is_ok() {
        let _ = fs::remove_dir_all(&old);
    }
    let _ = fs::remove_dir_all(&stage);
    done
}

/// The new app, downloaded, unpacked and checked, inside `stage`.
fn prepare(
    release: &Release,
    asset: &Asset,
    stage: &Path,
    progress: &impl Fn(Stage),
) -> Result<PathBuf, String> {
    let zip = stage.join("update.zip");
    progress(Stage::Downloading(0));
    download(&asset.url, &zip, asset.size, progress)?;
    progress(Stage::Checking);
    check_zip(&zip, asset)?;
    run(
        DITTO,
        [
            OsStr::new("-x"),
            OsStr::new("-k"),
            zip.as_os_str(),
            stage.as_os_str(),
        ],
    )?;
    let fresh = stage.join(APP);
    check_app(&fresh, &release.version)?;
    Ok(fresh)
}

/// Fetches `url` into `to` with curl, telling `progress` how far it is while it goes: `size` is what GitHub says
/// the file will be.
fn download(url: &str, to: &Path, size: u64, progress: &impl Fn(Stage)) -> Result<(), String> {
    // Loopback http is for the tests, as in `core_smart::net`; the address of a real release is always https.
    let protocol = if url.starts_with("http://127.0.0.1") {
        "=http"
    } else {
        "=https"
    };
    let mut curl = Command::new(CURL);
    curl.args(["--silent", "--show-error", "--fail", "--location"])
        .args(["--proto", protocol, "--proto-redir", protocol])
        .args(["--connect-timeout", "15"])
        .args(["--max-time", &DOWNLOAD_SECONDS.to_string()])
        // A connection that has stalled should not hold the update for the whole time limit.
        .args(["--speed-limit", "1024", "--speed-time", "30"])
        .args(["--max-filesize", &MAX_ZIP.to_string()])
        .arg("--output")
        .arg(to)
        .arg(url)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let mut child = curl.spawn().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            "curl is missing".to_owned()
        } else {
            format!("cannot start curl: {e}")
        }
    })?;
    let mut told = 0;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {}
            Err(e) => return Err(format!("cannot wait for curl: {e}")),
        }
        let arrived = fs::metadata(to).map_or(0, |m| m.len());
        // Never 100 until curl has finished: the last bytes are still to be checked.
        let percent =
            u8::try_from((arrived.saturating_mul(100) / size.max(1)).min(99)).unwrap_or(99);
        if percent != told {
            told = percent;
            progress(Stage::Downloading(percent));
        }
        std::thread::sleep(PROGRESS_INTERVAL);
    };
    match status.code() {
        Some(0) => Ok(()),
        // Could not resolve, could not connect, timed out, no response, TLS trouble.
        Some(5..=7 | 28 | 35 | 52 | 56 | 60) => Err("GitHub could not be reached".into()),
        Some(22) => Err("GitHub refused the download".into()),
        Some(23) => Err("the download could not be saved".into()),
        Some(63) => Err("the download is larger than expected".into()),
        Some(code) => Err(format!("curl exited with {code}")),
        None => Err("curl was stopped".into()),
    }
}

/// The zip is the size GitHub lists and, when it lists one, has the checksum GitHub computed.
fn check_zip(zip: &Path, asset: &Asset) -> Result<(), String> {
    let size = fs::metadata(zip)
        .map_err(|e| format!("cannot read the download: {e}"))?
        .len();
    if size != asset.size {
        return Err(format!("the download is {size} bytes, not {}", asset.size));
    }
    if let Some(expected) = &asset.sha256
        && sha256(zip)? != *expected
    {
        return Err("the download does not match the checksum on GitHub".into());
    }
    Ok(())
}

fn sha256(file: &Path) -> Result<String, String> {
    let out = run(
        SHASUM,
        [OsStr::new("-a"), OsStr::new("256"), file.as_os_str()],
    )?;
    out.split_whitespace()
        .next()
        .filter(|hash| hash.len() == 64)
        .map(str::to_lowercase)
        .ok_or_else(|| "shasum gave no checksum".to_owned())
}

/// An app of this program, of the version the release says, whose signature holds together.
fn check_app(app: &Path, version: &str) -> Result<(), String> {
    if !app.join("Contents/Info.plist").is_file() {
        return Err("the download is not an app".into());
    }
    if plist_value(app, "CFBundleIdentifier")? != BUNDLE_ID {
        return Err("the download is not Figghost".into());
    }
    let found = plist_value(app, "CFBundleShortVersionString")?;
    if found != version {
        return Err(format!("the download is version {found}, not {version}"));
    }
    if !app.join("Contents/MacOS/Figghost").is_file() {
        return Err("the download has no program in it".into());
    }
    run(
        CODESIGN,
        [
            OsStr::new("--verify"),
            OsStr::new("--strict"),
            app.as_os_str(),
        ],
    )
    .map(drop)
}

fn plist_value(app: &Path, key: &str) -> Result<String, String> {
    let plist = app.join("Contents/Info.plist");
    run(
        PLUTIL,
        [
            OsStr::new("-extract"),
            OsStr::new(key),
            OsStr::new("raw"),
            OsStr::new("-o"),
            OsStr::new("-"),
            plist.as_os_str(),
        ],
    )
}

/// Moves the installed app aside and the new one into its place. When the second move fails, the first is
/// undone, so there is always an app where `installed` is.
fn swap(installed: &Path, fresh: &Path, old: &Path) -> Result<(), String> {
    fs::rename(installed, old).map_err(|e| format!("cannot move the installed app aside: {e}"))?;
    if let Err(e) = fs::rename(fresh, installed) {
        let _ = fs::rename(old, installed);
        return Err(format!("cannot put the new app in place: {e}"));
    }
    Ok(())
}

/// Runs a system tool and gives what it printed, or the first line of what it complained about.
fn run<'a>(program: &str, args: impl IntoIterator<Item = &'a OsStr>) -> Result<String, String> {
    let output = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("cannot start {program}: {e}"))?;
    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned());
    }
    let name = Path::new(program)
        .file_name()
        .and_then(OsStr::to_str)
        .unwrap_or(program);
    let complaint = String::from_utf8_lossy(&output.stderr);
    let reason = complaint.lines().next().unwrap_or("no reason given");
    Err(format!("{name} failed: {reason}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        cell::RefCell,
        io::{BufRead, BufReader, Write},
        net::TcpListener,
        os::unix::fs::PermissionsExt,
        sync::atomic::{AtomicU32, Ordering},
        thread,
    };

    /// A folder of its own under the system's temporary one, removed at the end of the test.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new() -> Self {
            static NEXT: AtomicU32 = AtomicU32::new(0);
            let dir = std::env::temp_dir().join(format!(
                "figghost-update-test-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }

        fn path(&self, name: &str) -> PathBuf {
            self.0.join(name)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            // A test may have taken the right to write away from a folder.
            let _ = Command::new("/bin/chmod")
                .args(["-R", "u+rwx"])
                .arg(&self.0)
                .status();
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    /// A Figghost.app that passes every check: a real program (a copy of `true`, so that it can be signed),
    /// the identifier and `version`, signed ad hoc as the real one is.
    fn fake_app(folder: &Path, version: &str) -> PathBuf {
        let app = folder.join(APP);
        fs::create_dir_all(app.join("Contents/MacOS")).unwrap();
        fs::copy("/usr/bin/true", app.join("Contents/MacOS/Figghost")).unwrap();
        fs::write(
            app.join("Contents/Info.plist"),
            format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleIdentifier</key><string>{BUNDLE_ID}</string>
<key>CFBundleExecutable</key><string>Figghost</string>
<key>CFBundleShortVersionString</key><string>{version}</string>
<key>CFBundleVersion</key><string>{version}</string>
<key>CFBundlePackageType</key><string>APPL</string>
</dict></plist>"#
            ),
        )
        .unwrap();
        run(
            CODESIGN,
            [
                OsStr::new("--force"),
                OsStr::new("--sign"),
                OsStr::new("-"),
                app.as_os_str(),
            ],
        )
        .unwrap();
        app
    }

    /// Zips `item` the way `scripts/release.sh` zips the app.
    fn zip(item: &Path, to: &Path) {
        run(
            DITTO,
            [
                OsStr::new("-c"),
                OsStr::new("-k"),
                OsStr::new("--sequesterRsrc"),
                OsStr::new("--keepParent"),
                item.as_os_str(),
                to.as_os_str(),
            ],
        )
        .unwrap();
    }

    /// Hands `file` to the first request on a free loopback port; the address it is served at.
    fn serve(file: &Path) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!(
            "http://127.0.0.1:{}/Figghost.zip",
            listener.local_addr().unwrap().port()
        );
        let body = fs::read(file).unwrap();
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            loop {
                let mut line = String::new();
                if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
                    break;
                }
            }
            let head = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = stream.write_all(head.as_bytes());
            let _ = stream.write_all(&body);
        });
        url
    }

    /// Like `serve`, but the body comes in ten pieces with a pause before each.
    fn serve_slowly(file: &Path) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!(
            "http://127.0.0.1:{}/Figghost.zip",
            listener.local_addr().unwrap().port()
        );
        let body = fs::read(file).unwrap();
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            loop {
                let mut line = String::new();
                if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
                    break;
                }
            }
            let head = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = stream.write_all(head.as_bytes());
            for piece in body.chunks(body.len().div_ceil(10)) {
                thread::sleep(Duration::from_millis(120));
                let _ = stream.write_all(piece);
                let _ = stream.flush();
            }
        });
        url
    }

    fn release(version: &str, zip: &Path, sha256: Option<String>) -> Release {
        Release {
            version: version.into(),
            url: String::new(),
            asset: Some(Asset {
                url: serve(zip),
                size: fs::metadata(zip).unwrap().len(),
                sha256,
            }),
            changes: Vec::new(),
        }
    }

    /// An "Applications" folder holding a 1.0.0 that carries a marker, to tell it from what replaces it.
    fn installed(scratch: &Scratch) -> PathBuf {
        let folder = scratch.path("Applications");
        fs::create_dir_all(&folder).unwrap();
        let app = fake_app(&folder, "1.0.0");
        fs::write(app.join("Contents/old-marker"), "old").unwrap();
        app
    }

    fn version_of(app: &Path) -> String {
        plist_value(app, "CFBundleShortVersionString").unwrap()
    }

    #[test]
    fn the_app_around_the_program_is_found() {
        assert_eq!(
            bundle_of(Path::new(
                "/Applications/Figghost.app/Contents/MacOS/Figghost"
            )),
            Some(PathBuf::from("/Applications/Figghost.app"))
        );
        // A build folder is not an app, even when a folder above it happens to be named like one.
        assert_eq!(bundle_of(Path::new("/x/target/release/app-ui")), None);
        assert_eq!(
            bundle_of(Path::new("/x/Foo.app/target/release/app-ui")),
            None
        );
        assert_eq!(bundle_of(Path::new("/x/Foo/Contents/MacOS/Figghost")), None);
        assert_eq!(bundle_of(Path::new("app-ui")), None);
    }

    #[test]
    fn a_slow_download_reports_how_far_it_is() {
        let scratch = Scratch::new();
        let source = scratch.path("source.bin");
        fs::write(&source, vec![7u8; 100_000]).unwrap();
        let url = serve_slowly(&source);
        let target = scratch.path("target.bin");

        let told = RefCell::new(Vec::new());
        download(&url, &target, 100_000, &|stage: Stage| {
            told.borrow_mut()
                .push(stage.percent().expect("only the download reports"))
        })
        .unwrap();

        let told = told.into_inner();
        assert!(
            told.len() >= 2,
            "a download of over a second reports more than once: {told:?}"
        );
        assert!(
            told.windows(2).all(|pair| pair[0] < pair[1]),
            "it only moves forward: {told:?}"
        );
        assert!(
            told.iter().all(|&percent| percent <= 99),
            "it is not done until it is checked: {told:?}"
        );
        assert_eq!(fs::read(&target).unwrap().len(), 100_000);
    }

    #[test]
    fn the_checksum_of_a_file_is_read_from_shasum() {
        let scratch = Scratch::new();
        let file = scratch.path("abc");
        fs::write(&file, "abc").unwrap();
        assert_eq!(
            sha256(&file).unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert!(sha256(&scratch.path("missing")).is_err());
    }

    #[test]
    fn a_zip_must_be_the_size_and_have_the_checksum_that_github_lists() {
        let scratch = Scratch::new();
        let file = scratch.path("abc.zip");
        fs::write(&file, "abc").unwrap();
        let abc = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad".to_owned();
        let asset = |size, sha256| Asset {
            url: String::new(),
            size,
            sha256,
        };
        assert!(check_zip(&file, &asset(3, Some(abc.clone()))).is_ok());
        assert!(
            check_zip(&file, &asset(3, None)).is_ok(),
            "no checksum listed"
        );
        assert!(check_zip(&file, &asset(4, Some(abc))).is_err(), "the size");
        assert!(
            check_zip(&file, &asset(3, Some("0".repeat(64)))).is_err(),
            "the checksum"
        );
    }

    #[test]
    fn the_swap_puts_the_new_app_in_place_and_undoes_itself_when_it_cannot() {
        let scratch = Scratch::new();
        let (installed, old) = (scratch.path("Figghost.app"), scratch.path(OLD));
        fs::create_dir_all(&installed).unwrap();
        fs::write(installed.join("which"), "old").unwrap();
        let fresh = scratch.path("fresh/Figghost.app");
        fs::create_dir_all(&fresh).unwrap();
        fs::write(fresh.join("which"), "new").unwrap();

        swap(&installed, &fresh, &old).unwrap();
        assert_eq!(fs::read_to_string(installed.join("which")).unwrap(), "new");
        assert_eq!(fs::read_to_string(old.join("which")).unwrap(), "old");
        assert!(!fresh.exists());

        // There is nothing left to move in, so the move aside is taken back.
        fs::remove_dir_all(&old).unwrap();
        let error = swap(&installed, &scratch.path("nothing"), &old).unwrap_err();
        assert!(error.contains("cannot put the new app in place"), "{error}");
        assert_eq!(fs::read_to_string(installed.join("which")).unwrap(), "new");
        assert!(!old.exists());
    }

    #[test]
    fn tidy_removes_what_a_cut_short_update_left_but_never_the_last_copy() {
        let scratch = Scratch::new();
        let app = scratch.path("Figghost.app");
        let (stage, old) = (scratch.path(STAGE), scratch.path(OLD));
        for dir in [&stage, &old] {
            fs::create_dir_all(dir.join("inside")).unwrap();
        }

        tidy(&app);
        assert!(!stage.exists(), "the folder of the download");
        assert!(
            old.exists(),
            "no app is there, so the old copy may be the only one"
        );

        fs::create_dir_all(&app).unwrap();
        tidy(&app);
        assert!(!old.exists());
        assert!(app.exists());
    }

    #[test]
    fn a_release_is_downloaded_checked_and_put_in_place_of_the_installed_app() {
        let scratch = Scratch::new();
        let app = installed(&scratch);
        let built = scratch.path("built");
        fs::create_dir_all(&built).unwrap();
        let new = fake_app(&built, "2.0.0");
        let zip_file = scratch.path("Figghost-2.0.0.zip");
        zip(&new, &zip_file);
        let hash = sha256(&zip_file).unwrap();

        let stages = RefCell::new(Vec::new());
        install(&release("2.0.0", &zip_file, Some(hash)), &app, |s| {
            stages.borrow_mut().push(s)
        })
        .unwrap();

        assert_eq!(version_of(&app), "2.0.0");
        assert!(
            !app.join("Contents/old-marker").exists(),
            "the old files are gone"
        );
        let folder = app.parent().unwrap();
        assert!(!folder.join(STAGE).exists(), "the download is cleaned up");
        assert!(!folder.join(OLD).exists(), "the old version is cleaned up");
        // The download reports its percentages in between; what matters here is the order of the stages.
        let mut names: Vec<&str> = stages.borrow().iter().map(|s| s.name()).collect();
        names.dedup();
        assert_eq!(names, ["downloading", "checking", "installing"]);
        // And it is a whole app that opens as such.
        check_app(&app, "2.0.0").unwrap();
    }

    #[test]
    fn a_download_that_fails_a_check_leaves_the_installed_app_as_it_was() {
        let scratch = Scratch::new();
        let app = installed(&scratch);
        let built = scratch.path("built");
        fs::create_dir_all(&built).unwrap();
        let new = fake_app(&built, "3.0.0");
        let good = scratch.path("Figghost-3.0.0.zip");
        zip(&new, &good);
        let hash = sha256(&good).unwrap();

        let text = scratch.path("readme.txt");
        fs::write(&text, "not an app").unwrap();
        let not_an_app = scratch.path("readme.zip");
        zip(&text, &not_an_app);

        let cases = [
            (
                "the checksum",
                release("3.0.0", &good, Some("0".repeat(64))),
            ),
            ("the version", release("2.0.0", &good, Some(hash))),
            ("the content", release("3.0.0", &not_an_app, None)),
        ];
        for (what, release) in cases {
            let error = install(&release, &app, |_| {}).unwrap_err();
            println!("{what}: {error}");
            assert_eq!(version_of(&app), "1.0.0", "{what}");
            assert!(app.join("Contents/old-marker").exists(), "{what}");
            let folder = app.parent().unwrap();
            assert!(
                !folder.join(STAGE).exists(),
                "{what}: the download is cleaned up"
            );
            assert!(!folder.join(OLD).exists(), "{what}");
        }
    }

    #[test]
    fn a_folder_that_cannot_be_written_is_reported_before_anything_is_downloaded() {
        let scratch = Scratch::new();
        let app = installed(&scratch);
        let folder = app.parent().unwrap();
        fs::set_permissions(folder, fs::Permissions::from_mode(0o500)).unwrap();
        // No server is listening: reaching the download would be a different error.
        let release = Release {
            version: "2.0.0".into(),
            url: String::new(),
            asset: Some(Asset {
                url: "http://127.0.0.1:9/Figghost.zip".into(),
                size: 1,
                sha256: None,
            }),
            changes: Vec::new(),
        };
        let error = install(&release, &app, |_| {}).unwrap_err();
        assert!(error.starts_with("cannot write to"), "{error}");
        assert_eq!(version_of(&app), "1.0.0");
    }

    #[test]
    fn a_version_that_is_on_disk_already_is_not_downloaded_again() {
        let scratch = Scratch::new();
        let app = installed(&scratch);
        // Nothing is served at this address, and the release has no zip: any attempt to fetch would fail.
        let release = Release {
            version: "1.0.0".into(),
            url: String::new(),
            asset: None,
            changes: Vec::new(),
        };
        let touched = RefCell::new(false);
        install(&release, &app, |_| *touched.borrow_mut() = true).unwrap();
        assert!(!*touched.borrow(), "no stage was started");
        assert!(app.join("Contents/old-marker").exists());
    }

    #[test]
    fn a_release_without_an_app_cannot_be_installed() {
        let scratch = Scratch::new();
        let app = installed(&scratch);
        let release = Release {
            version: "2.0.0".into(),
            url: String::new(),
            asset: None,
            changes: Vec::new(),
        };
        assert!(install(&release, &app, |_| {}).is_err());
        assert_eq!(version_of(&app), "1.0.0");
    }
}
