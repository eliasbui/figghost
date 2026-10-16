//! What the shelf can do with a file: compress it, convert an image, make it smaller, remove its
//! background, read its text, show it in Finder.
//! The jobs run the tools that ship with macOS (`ditto` and `sips`) or the Vision framework next to the
//! original, and never change the original.

use std::{
    ffi::OsString,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

const JOB_TIMEOUT: Duration = Duration::from_secs(180);
/// Formats `sips` reads, and so the ones that can be converted.
const IMAGE_EXTENSIONS: [&str; 9] = [
    "png", "jpg", "jpeg", "heic", "heif", "tiff", "tif", "gif", "bmp",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Zip,
    ToPng,
    ToJpeg,
    ToHeic,
    Half,
    Quarter,
    RemoveBackground,
    CopyText,
    Reveal,
    Remove,
}

impl Action {
    /// The id the menu carries.
    pub const fn id(self) -> &'static str {
        match self {
            Action::Zip => "zip",
            Action::ToPng => "png",
            Action::ToJpeg => "jpeg",
            Action::ToHeic => "heic",
            Action::Half => "half",
            Action::Quarter => "quarter",
            Action::RemoveBackground => "remove_background",
            Action::CopyText => "copy_text",
            Action::Reveal => "reveal",
            Action::Remove => "remove",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        [
            Action::Zip,
            Action::ToPng,
            Action::ToJpeg,
            Action::ToHeic,
            Action::Half,
            Action::Quarter,
            Action::RemoveBackground,
            Action::CopyText,
            Action::Reveal,
            Action::Remove,
        ]
        .into_iter()
        .find(|a| a.id() == id)
    }
}

fn extension(path: &Path) -> String {
    path.extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

/// The actions that make sense for this file, in menu order. Converting to the format a file already has is
/// not offered.
pub fn available(path: &Path) -> Vec<Action> {
    let mut actions = vec![Action::Zip];
    let ext = extension(path);
    if IMAGE_EXTENSIONS.contains(&ext.as_str()) {
        if ext != "png" {
            actions.push(Action::ToPng);
        }
        if !matches!(ext.as_str(), "jpg" | "jpeg") {
            actions.push(Action::ToJpeg);
        }
        if !matches!(ext.as_str(), "heic" | "heif") {
            actions.push(Action::ToHeic);
        }
        actions.push(Action::Half);
        actions.push(Action::Quarter);
        // Needs macOS 14; older systems do not see the item at all.
        if macos_sys::vision::background_removal_available() {
            actions.push(Action::RemoveBackground);
        }
        actions.push(Action::CopyText);
    }
    actions.push(Action::Reveal);
    actions.push(Action::Remove);
    actions
}

/// `dir/stem.ext`, or `dir/stem 2.ext`, `dir/stem 3.ext`, ... when that name is taken.
pub fn unique_output(dir: &Path, stem: &str, ext: &str) -> PathBuf {
    let name = |n: u32| {
        let base = if n == 1 {
            stem.to_owned()
        } else {
            format!("{stem} {n}")
        };
        dir.join(format!("{base}.{ext}"))
    };
    (1..)
        .map(name)
        .find(|p| !p.exists())
        .unwrap_or_else(|| name(1))
}

/// A command that makes one new file.
#[derive(Debug, PartialEq, Eq)]
pub struct Job {
    program: &'static str,
    args: Vec<OsString>,
    pub output: PathBuf,
}

/// The command for an action on `source`; `None` for the actions that are not file jobs.
pub fn job(action: Action, source: &Path) -> Option<Job> {
    let dir = source.parent()?;
    let file_name = source.file_name()?.to_string_lossy().into_owned();
    let stem = source.file_stem()?.to_string_lossy().into_owned();
    let image = |format: &'static str, ext: &str| {
        let output = unique_output(dir, &stem, ext);
        Job {
            program: "/usr/bin/sips",
            args: vec![
                "-s".into(),
                "format".into(),
                format.into(),
                source.as_os_str().to_owned(),
                "--out".into(),
                output.as_os_str().to_owned(),
            ],
            output,
        }
    };
    Some(match action {
        Action::Zip => {
            let output = unique_output(dir, &file_name, "zip");
            Job {
                program: "/usr/bin/ditto",
                args: vec![
                    "-c".into(),
                    "-k".into(),
                    "--keepParent".into(),
                    source.as_os_str().to_owned(),
                    output.as_os_str().to_owned(),
                ],
                output,
            }
        }
        Action::ToPng => image("png", "png"),
        Action::ToJpeg => image("jpeg", "jpg"),
        Action::ToHeic => image("heic", "heic"),
        Action::Half
        | Action::Quarter
        | Action::RemoveBackground
        | Action::CopyText
        | Action::Reveal
        | Action::Remove => return None,
    })
}

/// The two smaller sizes the shelf offers: the shape is kept, both sides shrink.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Size {
    Half,
    Quarter,
}

impl Size {
    const fn percent(self) -> u32 {
        match self {
            Size::Half => 50,
            Size::Quarter => 25,
        }
    }

    /// The word the new file is named with.
    const fn word(self) -> &'static str {
        match self {
            Size::Half => "half",
            Size::Quarter => "quarter",
        }
    }
}

/// Both sides at `percent` of their size, rounded to the nearest pixel and never below one: a one pixel
/// image stays one pixel instead of disappearing.
pub fn scaled(width: u32, height: u32, percent: u32) -> (u32, u32) {
    let at = |side: u32| ((side * percent + 50) / 100).max(1);
    (at(width), at(height))
}

/// The pixel size of an image, read with `sips`; the same tool that writes the smaller copy.
fn image_size(source: &Path) -> Result<(u32, u32), String> {
    let out = Command::new("/usr/bin/sips")
        .args(["-g", "pixelWidth", "-g", "pixelHeight"])
        .arg(source)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("cannot start sips: {e}"))?;
    let text = String::from_utf8_lossy(&out.stdout);
    let side = |key: &str| {
        text.lines()
            .find_map(|l| l.trim().strip_prefix(&format!("{key}:")))
            .and_then(|v| v.trim().parse::<u32>().ok())
    };
    match (side("pixelWidth"), side("pixelHeight")) {
        (Some(w), Some(h)) if w > 0 && h > 0 => Ok((w, h)),
        _ => Err("the image cannot be read".into()),
    }
}

/// Writes `source` at the smaller size next to it and returns the new file. The format and the shape are
/// kept, and the original is never touched. A GIF comes out as a still picture, as it already does through
/// the convert actions: `sips` writes one image.
pub fn resize(source: &Path, size: Size) -> Result<PathBuf, String> {
    let ext = extension(source);
    if !IMAGE_EXTENSIONS.contains(&ext.as_str()) {
        return Err("the file is not an image".into());
    }
    let (width, height) = image_size(source)?;
    let (width, height) = scaled(width, height, size.percent());
    let dir = source.parent().ok_or("the file has no folder")?;
    let stem = source
        .file_stem()
        .ok_or("the file has no name")?
        .to_string_lossy()
        .into_owned();
    let output = unique_output(dir, &format!("{stem} {}", size.word()), &ext);
    let job = Job {
        program: "/usr/bin/sips",
        // The order sips wants: height first, then width.
        args: vec![
            "-z".into(),
            height.to_string().into(),
            width.to_string().into(),
            source.as_os_str().to_owned(),
            "--out".into(),
            output.as_os_str().to_owned(),
        ],
        output,
    };
    run(&job)?;
    Ok(job.output)
}

/// Largest image the shelf decodes itself: background removal and text recognition hold the pixels in
/// memory. The jobs that hand the file to `sips` have no such limit.
const MAX_IMAGE_BYTES: u64 = 64 * 1024 * 1024;

/// Writes the subject of `source` with a transparent background to a new PNG next to it and returns its
/// path. Never replaces a file: the name is taken with `create_new`.
pub fn cut_out(source: &Path) -> Result<PathBuf, String> {
    let dir = source.parent().ok_or("the file has no folder")?;
    let stem = source
        .file_stem()
        .ok_or("the file has no name")?
        .to_string_lossy()
        .into_owned();
    let size = fs::metadata(source).map_err(|e| e.to_string())?.len();
    if size > MAX_IMAGE_BYTES {
        return Err("the image is too large".into());
    }
    let png = macos_sys::vision::remove_background(&fs::read(source).map_err(|e| e.to_string())?)?;
    let stem = format!("{stem} no background");
    // Another job may take the name between the check and the write, so try the next one then.
    for _ in 0..16 {
        let output = unique_output(dir, &stem, "png");
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&output)
        {
            Ok(mut file) => {
                if let Err(e) = file.write_all(&png) {
                    let _ = fs::remove_file(&output);
                    return Err(e.to_string());
                }
                return Ok(output);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e.to_string()),
        }
    }
    Err("no free file name".into())
}

/// Runs the action on a file and returns the file it made.
pub fn make(action: Action, source: &Path) -> Result<PathBuf, String> {
    match action {
        Action::RemoveBackground => return cut_out(source),
        Action::Half => return resize(source, Size::Half),
        Action::Quarter => return resize(source, Size::Quarter),
        _ => {}
    }
    let job = job(action, source).ok_or("this action makes no file")?;
    run(&job)?;
    Ok(job.output)
}

/// The picture of `source`, for the action that reads the image instead of writing a file. The caller is on
/// the main thread when the text is recognised, which is where the answer comes back.
pub fn load_image(source: &Path) -> Result<macos_sys::screen_capture::Shot, String> {
    let size = fs::metadata(source).map_err(|e| e.to_string())?.len();
    if size > MAX_IMAGE_BYTES {
        return Err("the image is too large".into());
    }
    macos_sys::screen_capture::Shot::from_file(source)
}

/// Runs the job and waits for it, up to a few minutes. The new file exists when this returns `Ok`.
pub fn run(job: &Job) -> Result<(), String> {
    let mut child = Command::new(job.program)
        .args(&job.args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("cannot start {}: {e}", job.program))?;
    let started = Instant::now();
    loop {
        match child.try_wait().map_err(|e| e.to_string())? {
            Some(status) if status.success() && job.output.exists() => return Ok(()),
            Some(status) => {
                // Never leave half a file behind.
                let _ = std::fs::remove_file(&job.output);
                return Err(format!("{} failed ({status})", job.program));
            }
            None if started.elapsed() >= JOB_TIMEOUT => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = std::fs::remove_file(&job.output);
                return Err(format!("{} took too long", job.program));
            }
            None => thread::sleep(Duration::from_millis(40)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        env, fs,
        sync::atomic::{AtomicUsize, Ordering},
    };

    fn temp_dir() -> PathBuf {
        static N: AtomicUsize = AtomicUsize::new(0);
        let dir = env::temp_dir().join(format!(
            "figghost-shelf-actions-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A 1 by 1 pixel, 24 bit BMP: small enough to write out by hand, and a format `sips` reads.
    fn bmp() -> Vec<u8> {
        let mut file = Vec::new();
        file.extend(b"BM");
        file.extend(58u32.to_le_bytes()); // file size
        file.extend(0u32.to_le_bytes());
        file.extend(54u32.to_le_bytes()); // where the pixels start
        file.extend(40u32.to_le_bytes()); // header size
        file.extend(1i32.to_le_bytes()); // width
        file.extend(1i32.to_le_bytes()); // height
        file.extend(1u16.to_le_bytes()); // planes
        file.extend(24u16.to_le_bytes()); // bits per pixel
        file.extend(0u32.to_le_bytes()); // no compression
        file.extend(4u32.to_le_bytes()); // pixel data size: one row padded to 4 bytes
        file.extend(2835u32.to_le_bytes());
        file.extend(2835u32.to_le_bytes());
        file.extend(0u32.to_le_bytes());
        file.extend(0u32.to_le_bytes());
        file.extend([0x20, 0x80, 0xf0, 0x00]); // one pixel, blue green red, and the padding
        file
    }

    fn ids(actions: &[Action]) -> Vec<&'static str> {
        actions.iter().map(|a| a.id()).collect()
    }

    #[test]
    fn menus_follow_the_kind_of_file() {
        assert_eq!(
            ids(&available(Path::new("/x/report.pdf"))),
            ["zip", "reveal", "remove"]
        );
        assert_eq!(
            ids(&available(Path::new("/x/photo.JPG"))),
            [
                "zip",
                "png",
                "heic",
                "half",
                "quarter",
                "remove_background",
                "copy_text",
                "reveal",
                "remove"
            ]
        );
        assert_eq!(
            ids(&available(Path::new("/x/shot.png"))),
            [
                "zip",
                "jpeg",
                "heic",
                "half",
                "quarter",
                "remove_background",
                "copy_text",
                "reveal",
                "remove"
            ]
        );
        assert_eq!(
            ids(&available(Path::new("/x/live.heic"))),
            [
                "zip",
                "png",
                "jpeg",
                "half",
                "quarter",
                "remove_background",
                "copy_text",
                "reveal",
                "remove"
            ]
        );
        assert_eq!(
            ids(&available(Path::new("/x/folder"))),
            ["zip", "reveal", "remove"]
        );
    }

    #[test]
    fn action_ids_round_trip() {
        for a in [
            Action::Zip,
            Action::ToPng,
            Action::ToJpeg,
            Action::ToHeic,
            Action::Half,
            Action::Quarter,
            Action::RemoveBackground,
            Action::CopyText,
            Action::Reveal,
            Action::Remove,
        ] {
            assert_eq!(Action::from_id(a.id()), Some(a));
        }
        assert_eq!(Action::from_id("rm -rf"), None);
    }

    #[test]
    fn output_names_never_overwrite_anything() {
        let dir = temp_dir();
        assert_eq!(unique_output(&dir, "photo", "png"), dir.join("photo.png"));
        fs::write(dir.join("photo.png"), b"x").unwrap();
        assert_eq!(unique_output(&dir, "photo", "png"), dir.join("photo 2.png"));
        fs::write(dir.join("photo 2.png"), b"x").unwrap();
        assert_eq!(unique_output(&dir, "photo", "png"), dir.join("photo 3.png"));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn jobs_name_their_output_next_to_the_original() {
        let dir = temp_dir();
        let doc = dir.join("report.pdf");
        let zip = job(Action::Zip, &doc).unwrap();
        assert_eq!(zip.output, dir.join("report.pdf.zip"));
        let photo = dir.join("holiday.heic");
        let png = job(Action::ToPng, &photo).unwrap();
        assert_eq!(png.output, dir.join("holiday.png"));
        assert_eq!(
            job(Action::ToJpeg, &photo).unwrap().output,
            dir.join("holiday.jpg")
        );
        assert!(job(Action::Reveal, &photo).is_none() && job(Action::Remove, &photo).is_none());
        assert!(job(Action::RemoveBackground, &photo).is_none());
        assert!(job(Action::CopyText, &photo).is_none());
        assert!(
            job(Action::Half, &photo).is_none(),
            "a smaller size needs the size of the original first"
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn zipping_a_file_and_a_folder_really_works() {
        let dir = temp_dir();
        let file = dir.join("notes.txt");
        fs::write(&file, "hello shelf").unwrap();
        let zip = job(Action::Zip, &file).unwrap();
        run(&zip).unwrap();
        assert!(zip.output.metadata().unwrap().len() > 20);
        assert_eq!(
            fs::read_to_string(&file).unwrap(),
            "hello shelf",
            "the original is untouched"
        );
        // A second run does not replace the first archive.
        let again = job(Action::Zip, &file).unwrap();
        assert_eq!(again.output, dir.join("notes.txt 2.zip"));
        run(&again).unwrap();

        let folder = dir.join("Docs");
        fs::create_dir(&folder).unwrap();
        fs::write(folder.join("a.txt"), "a").unwrap();
        let zip = job(Action::Zip, &folder).unwrap();
        run(&zip).unwrap();
        let listing = Command::new("/usr/bin/unzip")
            .arg("-l")
            .arg(&zip.output)
            .output()
            .unwrap();
        let text = String::from_utf8_lossy(&listing.stdout);
        assert!(text.contains("Docs/a.txt"), "{text}");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn converting_an_image_makes_a_new_file_in_the_new_format() {
        let dir = temp_dir();
        let source = dir.join("pixel.bmp");
        fs::write(&source, bmp()).unwrap();
        let png = job(Action::ToPng, &source).unwrap();
        run(&png).unwrap();
        assert_eq!(
            &fs::read(&png.output).unwrap()[..4],
            &[0x89, b'P', b'N', b'G']
        );
        // And a converted file can be converted again.
        let jpeg = job(Action::ToJpeg, &png.output).unwrap();
        run(&jpeg).unwrap();
        assert_eq!(&fs::read(&jpeg.output).unwrap()[..3], &[0xff, 0xd8, 0xff]);
        assert_eq!(
            fs::read(&source).unwrap(),
            bmp(),
            "the original is untouched"
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn a_failed_job_leaves_no_partial_file_and_says_so() {
        let dir = temp_dir();
        let not_an_image = dir.join("fake.png");
        fs::write(&not_an_image, "this is not a picture").unwrap();
        let job = job(Action::ToJpeg, &not_an_image).unwrap();
        assert!(run(&job).is_err());
        assert!(!job.output.exists());
        let missing = super::job(Action::Zip, &dir.join("gone.txt")).unwrap();
        assert!(run(&missing).is_err());
        assert!(!missing.output.exists());
        let _ = fs::remove_dir_all(dir);
    }

    /// A 24 bit BMP of `side` pixels square: a red disc on a light grey ground.
    fn disc_bmp(side: u32) -> Vec<u8> {
        let row = (side * 3).div_ceil(4) * 4;
        let mut file = Vec::new();
        file.extend(b"BM");
        file.extend((54 + row * side).to_le_bytes());
        file.extend(0u32.to_le_bytes());
        file.extend(54u32.to_le_bytes());
        file.extend(40u32.to_le_bytes());
        file.extend((side as i32).to_le_bytes());
        file.extend((side as i32).to_le_bytes());
        file.extend(1u16.to_le_bytes());
        file.extend(24u16.to_le_bytes());
        file.extend(0u32.to_le_bytes());
        file.extend((row * side).to_le_bytes());
        file.extend([0u8; 16]);
        let (centre, radius) = (f64::from(side) / 2.0, f64::from(side) * 0.3);
        for y in 0..side {
            for x in 0..side {
                let (dx, dy) = (f64::from(x) - centre, f64::from(y) - centre);
                let pixel: [u8; 3] = if dx.hypot(dy) < radius {
                    [0x20, 0x20, 0xd8]
                } else {
                    [0xd8, 0xd8, 0xd8]
                };
                file.extend(pixel);
            }
            file.extend(vec![0u8; (row - side * 3) as usize]);
        }
        file
    }

    fn sips_value(path: &Path, key: &str) -> String {
        let out = Command::new("/usr/bin/sips")
            .args(["-g", key])
            .arg(path)
            .output()
            .unwrap();
        let text = String::from_utf8_lossy(&out.stdout).into_owned();
        text.lines()
            .find_map(|l| l.trim().strip_prefix(&format!("{key}:")))
            .unwrap_or_default()
            .trim()
            .to_owned()
    }

    #[test]
    fn removing_a_background_makes_a_new_transparent_png_and_keeps_the_original() {
        let dir = temp_dir();
        let source = dir.join("badge.bmp");
        fs::write(&source, disc_bmp(256)).unwrap();
        let made = make(Action::RemoveBackground, &source).unwrap();
        assert_eq!(made, dir.join("badge no background.png"));
        assert_eq!(&fs::read(&made).unwrap()[..4], &[0x89, b'P', b'N', b'G']);
        assert_eq!(sips_value(&made, "pixelWidth"), "256");
        assert_eq!(sips_value(&made, "pixelHeight"), "256");
        assert_eq!(sips_value(&made, "hasAlpha"), "yes");
        assert_eq!(
            fs::read(&source).unwrap(),
            disc_bmp(256),
            "the original is untouched"
        );
        // A second run does not replace the first result.
        let again = make(Action::RemoveBackground, &source).unwrap();
        assert_eq!(again, dir.join("badge no background 2.png"));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn the_formats_the_shelf_offers_all_go_through_background_removal() {
        let dir = temp_dir();
        let bmp = dir.join("disc.bmp");
        fs::write(&bmp, disc_bmp(256)).unwrap();
        for (action, ext) in [(Action::ToJpeg, "jpg"), (Action::ToHeic, "heic")] {
            let converted = make(action, &bmp).unwrap();
            assert_eq!(converted.extension().unwrap(), ext);
            let made = make(Action::RemoveBackground, &converted).unwrap();
            assert_eq!(sips_value(&made, "hasAlpha"), "yes", "{ext}");
            assert_eq!(sips_value(&made, "pixelWidth"), "256", "{ext}");
        }
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn background_removal_fails_cleanly_and_leaves_no_file() {
        let dir = temp_dir();
        let fake = dir.join("fake.png");
        fs::write(&fake, "this is not a picture").unwrap();
        assert!(cut_out(&fake).is_err());
        assert!(cut_out(&dir.join("gone.png")).is_err());
        assert!(make(Action::Reveal, &fake).is_err());
        let names: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, ["fake.png"]);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn a_smaller_size_keeps_the_shape_and_never_rounds_to_nothing() {
        assert_eq!(scaled(1920, 1080, 50), (960, 540));
        assert_eq!(scaled(1920, 1080, 25), (480, 270));
        assert_eq!(
            scaled(1921, 1081, 50),
            (961, 541),
            "an odd side is rounded, not dropped"
        );
        assert_eq!(scaled(3, 3, 50), (2, 2), "rounded to the nearest pixel");
        assert_eq!(scaled(1, 1, 25), (1, 1), "never zero pixels");
    }

    #[test]
    fn resizing_writes_a_smaller_copy_in_the_same_format_next_to_the_original() {
        let dir = temp_dir();
        let source = dir.join("badge.bmp");
        fs::write(&source, disc_bmp(256)).unwrap();
        let half = make(Action::Half, &source).unwrap();
        assert_eq!(half, dir.join("badge half.bmp"));
        assert_eq!(sips_value(&half, "pixelWidth"), "128");
        assert_eq!(sips_value(&half, "pixelHeight"), "128");
        let quarter = make(Action::Quarter, &source).unwrap();
        assert_eq!(quarter, dir.join("badge quarter.bmp"));
        assert_eq!(sips_value(&quarter, "pixelWidth"), "64");
        assert_eq!(
            fs::read(&source).unwrap(),
            disc_bmp(256),
            "the original is untouched"
        );
        // A second run does not replace the first result.
        assert_eq!(
            make(Action::Half, &source).unwrap(),
            dir.join("badge half 2.bmp")
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn resizing_something_that_is_not_an_image_fails_cleanly_and_leaves_no_file() {
        let dir = temp_dir();
        let notes = dir.join("notes.txt");
        fs::write(&notes, "not a picture").unwrap();
        assert!(
            resize(&notes, Size::Half).is_err(),
            "the kind of file is looked at before sips is asked"
        );
        let fake = dir.join("fake.png");
        fs::write(&fake, "not a picture").unwrap();
        assert!(make(Action::Half, &fake).is_err());
        assert!(make(Action::Quarter, &dir.join("gone.png")).is_err());
        let mut names: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        names.sort();
        assert_eq!(names, ["fake.png", "notes.txt"]);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn the_picture_of_a_file_can_be_read_and_a_file_of_another_kind_says_so() {
        let dir = temp_dir();
        let source = dir.join("disc.bmp");
        fs::write(&source, disc_bmp(64)).unwrap();
        let shot = load_image(&source).unwrap();
        assert_eq!((shot.width(), shot.height()), (64, 64));
        let fake = dir.join("fake.png");
        fs::write(&fake, "not a picture").unwrap();
        assert!(load_image(&fake).is_err());
        assert!(load_image(&dir.join("gone.png")).is_err());
        let _ = fs::remove_dir_all(dir);
    }
}
