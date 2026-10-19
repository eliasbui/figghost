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
    sync::atomic::{AtomicUsize, Ordering},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
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
    unique_output_avoiding(dir, stem, ext, &[])
}

/// Like `unique_output`, and also not a name in `taken`: the outputs of jobs of the same batch that have not
/// run yet. An empty `ext` names a folder, with no dot.
pub fn unique_output_avoiding(dir: &Path, stem: &str, ext: &str, taken: &[PathBuf]) -> PathBuf {
    let name = |n: u32| {
        let base = if n == 1 {
            stem.to_owned()
        } else {
            format!("{stem} {n}")
        };
        if ext.is_empty() {
            dir.join(base)
        } else {
            dir.join(format!("{base}.{ext}"))
        }
    };
    (1..)
        .map(name)
        .find(|p| !p.exists() && !taken.contains(p))
        .unwrap_or_else(|| name(1))
}

/// Work that is done in this process instead of by a command line tool, because macOS has no tool for it.
#[derive(Debug, PartialEq, Eq)]
pub enum Native {
    /// An image at a part of its size, in the same format.
    Resize { source: PathBuf, size: Size },
    /// An image without its metadata.
    StripImageMetadata { source: PathBuf },
    /// Several PDFs as one.
    PdfMerge { sources: Vec<PathBuf> },
    /// One PDF per page, in the folder `output`; the files are named after `stem`.
    PdfSplit { source: PathBuf, stem: String },
    /// One picture per page, in the folder `output`; the files are named after `stem`.
    PdfPages {
        source: PathBuf,
        stem: String,
        format: macos_sys::file_ops::PageImage,
    },
    /// A text document printed as a PDF.
    DocumentToPdf { source: PathBuf },
}

/// A job that makes one new file or folder: a command, or work done here.
#[derive(Debug, PartialEq, Eq)]
pub struct Job {
    program: &'static str,
    args: Vec<OsString>,
    pub output: PathBuf,
    native: Option<Native>,
}

impl Job {
    /// A command line tool run with these arguments (never through a shell) that must write `output`.
    pub fn command(program: &'static str, args: Vec<OsString>, output: PathBuf) -> Self {
        Self {
            program,
            args,
            output,
            native: None,
        }
    }

    /// Work done in this process that must write `output`.
    pub fn native(work: Native, output: PathBuf) -> Self {
        Self {
            program: "",
            args: Vec::new(),
            output,
            native: Some(work),
        }
    }

    #[cfg(test)]
    pub fn program(&self) -> &str {
        self.program
    }

    #[cfg(test)]
    pub fn args(&self) -> &[OsString] {
        &self.args
    }

    #[cfg(test)]
    pub fn native_work(&self) -> Option<&Native> {
        self.native.as_ref()
    }
}

/// The command for an action on `source`; `None` for the actions that are not file jobs.
pub fn job(action: Action, source: &Path) -> Option<Job> {
    let dir = source.parent()?;
    let file_name = source.file_name()?.to_string_lossy().into_owned();
    let stem = source.file_stem()?.to_string_lossy().into_owned();
    let image = |format: &'static str, ext: &str| {
        let output = unique_output(dir, &stem, ext);
        Job::command(
            "/usr/bin/sips",
            vec![
                "-s".into(),
                "format".into(),
                format.into(),
                source.as_os_str().to_owned(),
                "--out".into(),
                output.as_os_str().to_owned(),
            ],
            output,
        )
    };
    Some(match action {
        Action::Zip => {
            let output = unique_output(dir, &file_name, "zip");
            Job::command(
                "/usr/bin/ditto",
                vec![
                    "-c".into(),
                    "-k".into(),
                    "--keepParent".into(),
                    source.as_os_str().to_owned(),
                    output.as_os_str().to_owned(),
                ],
                output,
            )
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
    pub const fn percent(self) -> u32 {
        match self {
            Size::Half => 50,
            Size::Quarter => 25,
        }
    }

    /// The word the new file is named with.
    pub const fn word(self) -> &'static str {
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
    let dir = source.parent().ok_or("the file has no folder")?;
    let stem = source
        .file_stem()
        .ok_or("the file has no name")?
        .to_string_lossy()
        .into_owned();
    let output = unique_output(dir, &format!("{stem} {}", size.word()), &ext);
    resize_into(source, size, &output)
}

/// Writes `source` at the smaller size to `output`, which must be a new name. For the batch jobs of the drag
/// wheel, which choose the name before the job runs.
pub fn resize_into(source: &Path, size: Size, output: &Path) -> Result<PathBuf, String> {
    let (width, height) = image_size(source)?;
    let (width, height) = scaled(width, height, size.percent());
    let job = Job::command(
        "/usr/bin/sips",
        // The order sips wants: height first, then width.
        vec![
            "-z".into(),
            height.to_string().into(),
            width.to_string().into(),
            source.as_os_str().to_owned(),
            "--out".into(),
            output.as_os_str().to_owned(),
        ],
        output.to_owned(),
    );
    run_published(&job)
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
    run_published(&job)
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

/// Runs a command to its end, up to `JOB_TIMEOUT`, with no input and no output.
fn run_command(program: &str, args: &[OsString]) -> Result<(), String> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("cannot start {program}: {e}"))?;
    let started = Instant::now();
    loop {
        match child.try_wait().map_err(|e| e.to_string())? {
            Some(status) if status.success() => return Ok(()),
            Some(status) => return Err(format!("{program} failed ({status})")),
            None if started.elapsed() >= JOB_TIMEOUT => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("{program} took too long"));
            }
            None => thread::sleep(Duration::from_millis(40)),
        }
    }
}

/// Removes what a failed job left: a file, or the folder of a job that makes one.
fn remove_output(output: &Path) {
    if output.is_dir() {
        let _ = fs::remove_dir_all(output);
    } else {
        let _ = fs::remove_file(output);
    }
}

fn run_native(work: &Native, output: &Path) -> Result<(), String> {
    use macos_sys::file_ops as ops;
    match work {
        Native::Resize { source, size } => resize_into(source, *size, output).map(|_| ()),
        Native::StripImageMetadata { source } => ops::strip_image_metadata(source, output),
        Native::PdfMerge { sources } => ops::pdf_merge(sources, output),
        Native::PdfSplit { source, stem } => ops::pdf_split(source, output, stem).map(|_| ()),
        Native::PdfPages {
            source,
            stem,
            format,
        } => ops::pdf_to_images(source, output, stem, *format).map(|_| ()),
        Native::DocumentToPdf { source } => document_to_pdf(source, output),
    }
}

/// A document of any type `textutil` reads becomes RTF first (reading HTML in this process would need the main
/// thread), and the RTF is printed to the PDF.
fn document_to_pdf(source: &Path, output: &Path) -> Result<(), String> {
    if extension(source) == "rtf" {
        return macos_sys::file_ops::rtf_to_pdf(source, output);
    }
    let middle = output.with_extension("figghost-tmp.rtf");
    let made = run_command(
        "/usr/bin/textutil",
        &[
            "-convert".into(),
            "rtf".into(),
            source.as_os_str().to_owned(),
            "-output".into(),
            middle.as_os_str().to_owned(),
        ],
    )
    .and_then(|()| macos_sys::file_ops::rtf_to_pdf(&middle, output));
    let _ = fs::remove_file(&middle);
    made
}

/// A name for the tool to write to, in the folder of the final name and with its extension (`avconvert` reads
/// the format from it), that nobody else picks: it is hidden and has this process, a counter and the time in it.
fn staging_path(output: &Path) -> PathBuf {
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let dir = output.parent().unwrap_or(Path::new("."));
    let ext = output
        .extension()
        .map(|e| e.to_string_lossy().into_owned())
        .filter(|e| e.len() <= 8 && e.chars().all(|c| c.is_ascii_alphanumeric()));
    loop {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.subsec_nanos());
        let name = format!(
            ".figghost-{}-{}-{nanos}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        );
        let path = match &ext {
            Some(ext) => dir.join(format!("{name}.{ext}")),
            None => dir.join(name),
        };
        if !path.exists() {
            return path;
        }
    }
}

/// Moves the finished `staged` file or folder to `wanted`, or to the next free name when `wanted` is taken, and
/// never replaces anything: a file is linked under the new name and the staged name is then removed (a link
/// fails when the name exists), a folder is renamed with the exclusive flag. Returns the name it ended up under.
fn publish(staged: &Path, wanted: &Path) -> Result<PathBuf, String> {
    let dir = wanted.parent().ok_or("the result has no folder")?;
    let is_dir = staged.is_dir();
    let name = wanted
        .file_name()
        .ok_or("the result has no name")?
        .to_string_lossy()
        .into_owned();
    // A folder keeps its whole name; for a file the extension stays after the number.
    let (stem, ext) = if is_dir {
        (name.clone(), String::new())
    } else {
        (
            wanted
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or(name.clone()),
            extension(wanted),
        )
    };
    let mut target = wanted.to_owned();
    // Someone may take each free name in the meantime, so try again with the next, up to a bound.
    for _ in 0..1000 {
        let placed = if is_dir {
            macos_sys::file_ops::rename_exclusive(staged, &target)
        } else {
            match fs::hard_link(staged, &target) {
                Ok(()) => {
                    let _ = fs::remove_file(staged);
                    Ok(())
                }
                // A volume without links still has the exclusive rename.
                Err(e) if e.kind() != std::io::ErrorKind::AlreadyExists => {
                    macos_sys::file_ops::rename_exclusive(staged, &target)
                }
                Err(e) => Err(e),
            }
        };
        match placed {
            Ok(()) => return Ok(target),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                target = unique_output(dir, &stem, &ext);
            }
            Err(e) => return Err(e.to_string()),
        }
    }
    Err("no free file name".into())
}

/// Runs the job and waits for it, up to a few minutes. The new file or folder exists when this returns `Ok`;
/// when it fails nothing it started to write is left behind.
#[allow(dead_code)]
pub fn run(job: &Job) -> Result<(), String> {
    run_published(job).map(|_| ())
}

/// Like `run`, and returns the name the result was published under. The tool writes to a hidden staging name in
/// the same folder, and only a finished result is moved to the planned name, or to the next free one when that
/// name was taken in the meantime (the name is chosen when the slot is dropped, and a job can run for minutes).
/// So nothing that exists is ever replaced, and a failed job leaves no staged file behind.
pub fn run_published(job: &Job) -> Result<PathBuf, String> {
    let staged = staging_path(&job.output);
    let done = match &job.native {
        Some(work) => run_native(work, &staged),
        None => {
            let args: Vec<OsString> = job
                .args
                .iter()
                .map(|arg| {
                    if arg.as_os_str() == job.output.as_os_str() {
                        staged.as_os_str().to_owned()
                    } else {
                        arg.clone()
                    }
                })
                .collect();
            run_command(job.program, &args)
        }
    };
    let published = match done {
        Ok(()) if staged.exists() => publish(&staged, &job.output),
        Ok(()) => Err("the job made no file".into()),
        Err(e) => Err(e),
    };
    if published.is_err() {
        remove_output(&staged);
    }
    published
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

    /// What a folder holds, hidden names included, sorted: a staged file left behind would show here.
    fn listing(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    /// The first QA case (B-1): the name is chosen when the slot is dropped, and a file can appear under it
    /// before the job runs. The job must never replace it.
    #[test]
    fn a_name_taken_before_the_job_runs_is_never_overwritten() {
        let dir = temp_dir();
        let source = dir.join("pixel.bmp");
        fs::write(&source, bmp()).unwrap();
        let planned = job(Action::ToPng, &source).unwrap();
        assert_eq!(planned.output, dir.join("pixel.png"));
        fs::write(&planned.output, "precious").unwrap();
        run(&planned).unwrap();
        assert_eq!(
            fs::read_to_string(dir.join("pixel.png")).unwrap(),
            "precious",
            "the file that was there is untouched"
        );
        // The picture is under the next free name, and nothing else is left in the folder.
        let made = dir.join("pixel 2.png");
        assert_eq!(&fs::read(&made).unwrap()[..4], &[0x89, b'P', b'N', b'G']);
        assert_eq!(listing(&dir), ["pixel 2.png", "pixel.bmp", "pixel.png"]);
        let _ = fs::remove_dir_all(dir);
    }

    /// Two drops of the same file while the first job runs plan the same name; both results must survive.
    #[test]
    fn two_jobs_planned_before_either_runs_each_keep_their_result() {
        let dir = temp_dir();
        let source = dir.join("pixel.bmp");
        fs::write(&source, bmp()).unwrap();
        let (first, second) = (
            job(Action::ToJpeg, &source).unwrap(),
            job(Action::ToJpeg, &source).unwrap(),
        );
        assert_eq!(first.output, second.output, "the plans share a name");
        run(&first).unwrap();
        let first_bytes = fs::read(&first.output).unwrap();
        run(&second).unwrap();
        assert_eq!(
            fs::read(&first.output).unwrap(),
            first_bytes,
            "the first result is not replaced by the second"
        );
        assert_eq!(listing(&dir), ["pixel 2.jpg", "pixel.bmp", "pixel.jpg"]);
        let _ = fs::remove_dir_all(dir);
    }

    /// A folder that appears under the planned name (even an empty one, which a plain rename would replace) is
    /// left alone, and the new folder takes the next name.
    #[test]
    fn a_folder_result_never_replaces_a_folder_that_is_already_there() {
        let dir = temp_dir();
        let source = dir.join("pixel.bmp");
        fs::write(&source, bmp()).unwrap();
        let pdf = job_pdf(&dir, &source);
        let planned = dir.join("pixel pages");
        fs::create_dir(&planned).unwrap();
        let work = Job::native(
            Native::PdfSplit {
                source: pdf,
                stem: "pixel".into(),
            },
            planned.clone(),
        );
        run(&work).unwrap();
        assert_eq!(
            fs::read_dir(&planned).unwrap().count(),
            0,
            "the empty folder that was there is still empty"
        );
        assert_eq!(
            fs::read_dir(dir.join("pixel pages 2")).unwrap().count(),
            1,
            "the pages are in the next free folder"
        );
        let _ = fs::remove_dir_all(dir);
    }

    fn job_pdf(dir: &Path, source: &Path) -> PathBuf {
        let made = job(Action::ToPng, source).unwrap();
        run(&made).unwrap();
        let pdf = dir.join("one page.pdf");
        let status = Command::new("/usr/bin/sips")
            .args(["-s", "format", "pdf"])
            .arg(&made.output)
            .arg("--out")
            .arg(&pdf)
            .output()
            .unwrap();
        assert!(status.status.success());
        pdf
    }

    #[test]
    fn a_failed_job_leaves_no_staged_file_or_folder() {
        let dir = temp_dir();
        let fake = dir.join("fake.png");
        fs::write(&fake, "this is not a picture").unwrap();
        let converting = job(Action::ToJpeg, &fake).unwrap();
        assert!(run(&converting).is_err());
        let not_pdf = dir.join("fake.pdf");
        fs::write(&not_pdf, "not a pdf").unwrap();
        let splitting = Job::native(
            Native::PdfSplit {
                source: not_pdf,
                stem: "fake".into(),
            },
            dir.join("fake split"),
        );
        assert!(run(&splitting).is_err());
        assert_eq!(listing(&dir), ["fake.pdf", "fake.png"]);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn the_name_that_was_published_is_the_name_that_is_reported() {
        let dir = temp_dir();
        let source = dir.join("pixel.bmp");
        fs::write(&source, bmp()).unwrap();
        let (first, second) = (
            job(Action::ToPng, &source).unwrap(),
            job(Action::ToPng, &source).unwrap(),
        );
        assert_eq!(run_published(&first).unwrap(), dir.join("pixel.png"));
        assert_eq!(run_published(&second).unwrap(), dir.join("pixel 2.png"));
        let _ = fs::remove_dir_all(dir);
    }
}
