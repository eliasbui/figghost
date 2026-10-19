//! What the drag wheel does to files that no command line tool that ships with macOS can do: strip the metadata
//! of an image (ImageIO), merge, split and draw PDF pages (CoreGraphics), print a rich text document to a PDF
//! (AppKit text layout), put file URLs on the pasteboard, send files with AirDrop and open Terminal at a folder.
//!
//! Everything that writes a file writes only the path it is given, and the caller picks a free name; a failed
//! call may leave a partial file, which the caller removes.

use objc2::{
    AnyThread, MainThreadMarker,
    rc::{Retained, autoreleasepool},
    runtime::{AnyObject, ProtocolObject},
};
use objc2_app_kit::{
    NSAttributedStringAppKitDocumentFormats, NSGraphicsContext, NSLayoutManager, NSPasteboard,
    NSPasteboardWriting, NSSharingService, NSSharingServiceNameSendViaAirDrop, NSTextContainer,
    NSTextStorage,
};
use objc2_core_foundation::{
    CFDictionary, CFRetained, CFString, CFType, CFURL, CGAffineTransform, CGPoint, CGRect, CGSize,
};
use objc2_core_graphics::{
    CGBitmapContextCreate, CGBitmapContextCreateImage, CGColorSpace, CGContext, CGImageAlphaInfo,
    CGPDFBox, CGPDFContextBeginPage, CGPDFContextClose, CGPDFContextCreateWithURL,
    CGPDFContextEndPage, CGPDFDocument, CGPDFPage, kCGPDFContextMediaBox,
};
use objc2_foundation::{
    NSArray, NSData, NSDictionary, NSMutableDictionary, NSNumber, NSRange, NSString, NSURL,
};
use objc2_image_io::{
    CGImageDestination, CGImageSource, kCGImageDestinationLossyCompressionQuality,
    kCGImageDestinationMergeMetadata, kCGImageDestinationMetadata, kCGImagePropertyDPIHeight,
    kCGImagePropertyDPIWidth, kCGImagePropertyOrientation,
};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    ptr,
};

/// The pixel density pages are drawn at when they become pictures.
pub const PAGE_DPI: f64 = 300.0;
/// A page that would need more pixels than this on a side is refused instead of exhausting memory.
const MAX_SIDE_PIXELS: f64 = 20_000.0;

fn file_url(path: &Path) -> Retained<NSURL> {
    NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy()))
}

fn cf_url(url: &NSURL) -> &CFURL {
    // SAFETY: NSURL and CFURL are toll-free bridged.
    unsafe { &*(url as *const NSURL as *const CFURL) }
}

fn cf_string(string: &NSString) -> &CFString {
    // SAFETY: NSString and CFString are toll-free bridged.
    unsafe { &*(string as *const NSString as *const CFString) }
}

fn cf_dictionary<K, V>(dictionary: &NSDictionary<K, V>) -> &CFDictionary {
    // SAFETY: NSDictionary and CFDictionary are toll-free bridged.
    unsafe { &*(dictionary as *const NSDictionary<K, V> as *const CFDictionary) }
}

fn ns_string(key: &CFString) -> &NSString {
    // SAFETY: the same bridging, for the constant keys ImageIO and CoreGraphics define.
    unsafe { &*(key as *const CFString as *const NSString) }
}

// -- images ---------------------------------------------------------------------------------------------------

/// The orientation tag of the first picture, 1 (upright) when the file has none.
fn orientation(source: &CGImageSource) -> i32 {
    // SAFETY: the source is valid; the dictionary is read as a bridged NSDictionary.
    unsafe {
        source
            .properties_at_index(0, None)
            .and_then(|p| {
                let dictionary =
                    &*(&*p as *const CFDictionary as *const NSDictionary<NSString, AnyObject>);
                let value = dictionary.objectForKey(ns_string(kCGImagePropertyOrientation))?;
                Some(value.downcast_ref::<NSNumber>()?.intValue())
            })
            .filter(|o| (1..=8).contains(o))
            .unwrap_or(1)
    }
}

/// The metadata (as a bridged dictionary of dictionaries) of the first picture of a file, for checks.
pub fn image_properties(path: &Path) -> Option<Retained<NSDictionary<NSString, AnyObject>>> {
    let url = file_url(path);
    // SAFETY: the URL is valid for the call; the copy of the properties is retained before it is returned.
    unsafe {
        let source = CGImageSource::with_url(cf_url(&url), None)?;
        let properties = source.properties_at_index(0, None)?;
        let dictionary =
            &*(&*properties as *const CFDictionary as *const NSDictionary<NSString, AnyObject>);
        Retained::retain(dictionary as *const _ as *mut NSDictionary<NSString, AnyObject>)
    }
}

/// Writes the first picture of `source` to `output` in the same format with no metadata but the orientation:
/// no GPS, Exif, TIFF, maker or XMP data. A JPEG is copied without being compressed again; ImageIO's copy
/// keeps every tag of the other formats, so those are decoded and written afresh (lossless for PNG and TIFF,
/// at high quality for HEIC). AVIF is not handled: its writer drops the orientation.
pub fn strip_image_metadata(source: &Path, output: &Path) -> Result<(), String> {
    autoreleasepool(|_| {
        let (from, to) = (file_url(source), file_url(output));
        // SAFETY: every pointer handed over below is valid for the call; the error out-pointer is null.
        unsafe {
            let image =
                CGImageSource::with_url(cf_url(&from), None).ok_or("the image cannot be read")?;
            let kind = image.r#type().ok_or("the image has no known format")?;
            if image.count() == 0 {
                return Err("the image has no picture".into());
            }
            let turn = NSNumber::new_i32(orientation(&image));
            let destination = CGImageDestination::with_url(cf_url(&to), &kind, 1, None)
                .ok_or("the image cannot be written in this format")?;
            if kind.to_string() == "public.jpeg" {
                let metadata = objc2_image_io::CGMutableImageMetadata::new();
                let options: Retained<NSMutableDictionary<NSString, AnyObject>> =
                    NSMutableDictionary::new();
                let metadata_ref: &AnyObject = &*(&*metadata
                    as *const objc2_image_io::CGMutableImageMetadata
                    as *const AnyObject);
                options.setObject_forKey(
                    metadata_ref,
                    ProtocolObject::from_ref(ns_string(kCGImageDestinationMetadata)),
                );
                // Replace the metadata instead of merging into it; this is what removes the old tags.
                options.setObject_forKey(
                    &*NSNumber::new_bool(false),
                    ProtocolObject::from_ref(ns_string(kCGImageDestinationMergeMetadata)),
                );
                // The orientation is a tag like the others, so it is put back by hand: the pixels stay
                // upright only with it.
                let value = &*(&*turn as *const NSNumber as *const CFType);
                metadata.set_value_with_path(
                    None,
                    cf_string(&NSString::from_str("tiff:Orientation")),
                    value,
                );
                if !destination.copy_image_source(
                    &image,
                    Some(cf_dictionary(&options)),
                    ptr::null_mut(),
                ) {
                    return Err("the metadata could not be removed".into());
                }
            } else {
                let picture = image
                    .image_at_index(0, None)
                    .ok_or("the picture cannot be decoded")?;
                let properties: Retained<NSMutableDictionary<NSString, AnyObject>> =
                    NSMutableDictionary::new();
                properties.setObject_forKey(
                    &*turn,
                    ProtocolObject::from_ref(ns_string(kCGImagePropertyOrientation)),
                );
                properties.setObject_forKey(
                    &*NSNumber::new_f64(0.95),
                    ProtocolObject::from_ref(ns_string(kCGImageDestinationLossyCompressionQuality)),
                );
                destination.add_image(&picture, Some(cf_dictionary(&properties)));
                if !destination.finalize() {
                    return Err("the picture cannot be written in this format".into());
                }
            }
        }
        Ok(())
    })
}

// -- PDF --------------------------------------------------------------------------------------------------------

fn open_pdf(path: &Path) -> Result<CFRetained<CGPDFDocument>, String> {
    let url = file_url(path);
    let document = CGPDFDocument::with_url(Some(cf_url(&url))).ok_or("the PDF cannot be read")?;
    if CGPDFDocument::is_encrypted(Some(&document)) && !CGPDFDocument::is_unlocked(Some(&document))
    {
        return Err("the PDF is protected by a password".into());
    }
    if CGPDFDocument::number_of_pages(Some(&document)) == 0 {
        return Err("the PDF has no pages".into());
    }
    Ok(document)
}

pub fn pdf_page_count(path: &Path) -> Result<usize, String> {
    open_pdf(path).map(|d| CGPDFDocument::number_of_pages(Some(&d)))
}

/// The page as it shows: its media box, turned by its rotation.
fn page_rect(page: &CGPDFPage) -> CGRect {
    let media = CGPDFPage::box_rect(Some(page), CGPDFBox::MediaBox);
    let turned = CGPDFPage::rotation_angle(Some(page)).rem_euclid(180) == 90;
    let (width, height) = if turned {
        (media.size.height, media.size.width)
    } else {
        (media.size.width, media.size.height)
    };
    CGRect {
        origin: CGPoint { x: 0.0, y: 0.0 },
        size: CGSize { width, height },
    }
}

/// How to turn a page so that it fills `rect`, as the page shows.
fn page_transform(page: &CGPDFPage, rect: CGRect) -> CGAffineTransform {
    CGPDFPage::drawing_transform(Some(page), CGPDFBox::MediaBox, rect, 0, true)
}

struct PdfWriter {
    context: CFRetained<CGContext>,
}

impl PdfWriter {
    fn create(path: &Path) -> Result<Self, String> {
        let url = file_url(path);
        // SAFETY: the URL is valid; a null media box and no auxiliary info are allowed.
        let context = unsafe { CGPDFContextCreateWithURL(Some(cf_url(&url)), ptr::null(), None) }
            .ok_or("the PDF cannot be created")?;
        Ok(Self { context })
    }

    /// Starts a page of the given size and returns once the caller may draw.
    fn begin(&self, size: CGSize) {
        let rect = CGRect {
            origin: CGPoint { x: 0.0, y: 0.0 },
            size,
        };
        let info: Retained<NSMutableDictionary<NSString, AnyObject>> = NSMutableDictionary::new();
        // SAFETY: the media box key is a process-lifetime constant; CoreGraphics takes its value as the bytes
        // of a CGRect.
        unsafe {
            let data = NSData::with_bytes(core::slice::from_raw_parts(
                &rect as *const CGRect as *const u8,
                core::mem::size_of::<CGRect>(),
            ));
            info.setObject_forKey(
                &*data,
                ProtocolObject::from_ref(ns_string(kCGPDFContextMediaBox)),
            );
            CGPDFContextBeginPage(Some(&self.context), Some(cf_dictionary(&info)));
        }
    }

    fn end(&self) {
        CGPDFContextEndPage(Some(&self.context));
    }

    fn finish(self) {
        CGPDFContextClose(Some(&self.context));
    }
}

/// Appends every page of `source` to `writer`, vector content kept.
fn copy_pages(document: &CGPDFDocument, writer: &PdfWriter, only: Option<usize>) {
    let total = CGPDFDocument::number_of_pages(Some(document));
    for number in 1..=total {
        if only.is_some_and(|n| n != number) {
            continue;
        }
        let Some(page) = CGPDFDocument::page(Some(document), number) else {
            continue;
        };
        let rect = page_rect(&page);
        writer.begin(rect.size);
        CGContext::save_g_state(Some(&writer.context));
        CGContext::concat_ctm(Some(&writer.context), page_transform(&page, rect));
        CGContext::draw_pdf_page(Some(&writer.context), Some(&page));
        CGContext::restore_g_state(Some(&writer.context));
        writer.end();
    }
}

/// One PDF holding the pages of every source, in the order given.
pub fn pdf_merge(sources: &[PathBuf], output: &Path) -> Result<(), String> {
    if sources.len() < 2 {
        return Err("merging needs two or more PDFs".into());
    }
    autoreleasepool(|_| {
        // Open them all first, so that a bad file fails before anything is written.
        let documents = sources
            .iter()
            .map(|s| open_pdf(s))
            .collect::<Result<Vec<_>, _>>()?;
        let writer = PdfWriter::create(output)?;
        for document in &documents {
            copy_pages(document, &writer, None);
        }
        writer.finish();
        Ok(())
    })
}

/// One PDF per page in `dir` (which must not exist yet and is created), named `"<stem> <number>.pdf"`.
/// Returns how many it wrote.
pub fn pdf_split(source: &Path, dir: &Path, stem: &str) -> Result<usize, String> {
    autoreleasepool(|_| {
        let document = open_pdf(source)?;
        let total = CGPDFDocument::number_of_pages(Some(&document));
        fs::create_dir(dir).map_err(|e| e.to_string())?;
        let width = total.to_string().len();
        for number in 1..=total {
            let writer = PdfWriter::create(&dir.join(format!("{stem} {number:0width$}.pdf")))?;
            copy_pages(&document, &writer, Some(number));
            writer.finish();
        }
        Ok(total)
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageImage {
    Png,
    Jpeg,
}

impl PageImage {
    pub const fn extension(self) -> &'static str {
        match self {
            PageImage::Png => "png",
            PageImage::Jpeg => "jpg",
        }
    }

    const fn uti(self) -> &'static str {
        match self {
            PageImage::Png => "public.png",
            PageImage::Jpeg => "public.jpeg",
        }
    }
}

/// Every page of `source` as a picture at `PAGE_DPI`, in `dir` (created; it must not exist yet), named
/// `"<stem> <number>.<ext>"`. Returns how many it wrote.
pub fn pdf_to_images(
    source: &Path,
    dir: &Path,
    stem: &str,
    format: PageImage,
) -> Result<usize, String> {
    let document = autoreleasepool(|_| open_pdf(source))?;
    let total = CGPDFDocument::number_of_pages(Some(&document));
    fs::create_dir(dir).map_err(|e| e.to_string())?;
    let width = total.to_string().len();
    let scale = PAGE_DPI / 72.0;
    for number in 1..=total {
        // One pass per page, so the pixels of the last page are released before the next is drawn.
        autoreleasepool(|_| {
            let page = CGPDFDocument::page(Some(&document), number).ok_or("a page is missing")?;
            let rect = page_rect(&page);
            let (pixels_wide, pixels_high) = (rect.size.width * scale, rect.size.height * scale);
            if pixels_wide > MAX_SIDE_PIXELS || pixels_high > MAX_SIDE_PIXELS {
                return Err("a page is too large to draw at this density".to_string());
            }
            let (w, h) = (
                (pixels_wide.round() as usize).max(1),
                (pixels_high.round() as usize).max(1),
            );
            let space = CGColorSpace::new_device_rgb().ok_or("no colour space")?;
            // SAFETY: null data lets CoreGraphics own the memory; the sizes were checked above.
            let context = unsafe {
                CGBitmapContextCreate(
                    ptr::null_mut(),
                    w,
                    h,
                    8,
                    0,
                    Some(&space),
                    CGImageAlphaInfo::PremultipliedLast.0,
                )
            }
            .ok_or("the page does not fit in memory")?;
            // A page is white paper; a JPEG could not keep a transparent ground anyway.
            CGContext::set_rgb_fill_color(Some(&context), 1.0, 1.0, 1.0, 1.0);
            CGContext::fill_rect(
                Some(&context),
                CGRect {
                    origin: CGPoint { x: 0.0, y: 0.0 },
                    size: CGSize {
                        width: w as f64,
                        height: h as f64,
                    },
                },
            );
            CGContext::scale_ctm(
                Some(&context),
                w as f64 / rect.size.width,
                h as f64 / rect.size.height,
            );
            CGContext::concat_ctm(Some(&context), page_transform(&page, rect));
            CGContext::draw_pdf_page(Some(&context), Some(&page));
            let image =
                CGBitmapContextCreateImage(Some(&context)).ok_or("the page cannot be drawn")?;
            let out =
                file_url(&dir.join(format!("{stem} {number:0width$}.{}", format.extension())));
            // SAFETY: the URL and type are valid; the properties are plain numbers.
            unsafe {
                let destination = CGImageDestination::with_url(
                    cf_url(&out),
                    cf_string(&NSString::from_str(format.uti())),
                    1,
                    None,
                )
                .ok_or("the picture cannot be created")?;
                let properties: Retained<NSMutableDictionary<NSString, AnyObject>> =
                    NSMutableDictionary::new();
                for key in [kCGImagePropertyDPIWidth, kCGImagePropertyDPIHeight] {
                    properties.setObject_forKey(
                        &*NSNumber::new_f64(PAGE_DPI),
                        ProtocolObject::from_ref(ns_string(key)),
                    );
                }
                properties.setObject_forKey(
                    &*NSNumber::new_f64(0.9),
                    ProtocolObject::from_ref(ns_string(kCGImageDestinationLossyCompressionQuality)),
                );
                destination.add_image(&image, Some(cf_dictionary(&properties)));
                if !destination.finalize() {
                    return Err("the picture cannot be written".to_string());
                }
            }
            Ok(())
        })?;
    }
    Ok(total)
}

// -- rich text to PDF -----------------------------------------------------------------------------------------

/// US Letter in points, with a one inch margin all round: the layout of a printed page.
const PAGE: CGSize = CGSize {
    width: 612.0,
    height: 792.0,
};
const MARGIN: f64 = 72.0;

/// Prints an RTF file as a PDF with pages of US Letter. Other document types are first turned into RTF by
/// `textutil`, because reading HTML would need the main thread.
pub fn rtf_to_pdf(source: &Path, output: &Path) -> Result<(), String> {
    autoreleasepool(|_| {
        let bytes = fs::read(source).map_err(|e| e.to_string())?;
        let data = NSData::with_bytes(&bytes);
        // SAFETY: the data is valid RTF or the call returns None.
        let text = unsafe {
            objc2_foundation::NSAttributedString::initWithRTF_documentAttributes(
                objc2_foundation::NSAttributedString::alloc(),
                &data,
                None,
            )
        }
        .ok_or("the document cannot be read")?;
        let storage = NSTextStorage::new();
        storage.setAttributedString(&text);
        let layout = NSLayoutManager::new();
        storage.addLayoutManager(&layout);
        let inner = CGSize {
            width: PAGE.width - 2.0 * MARGIN,
            height: PAGE.height - 2.0 * MARGIN,
        };
        let mut containers: Vec<Retained<NSTextContainer>> = Vec::new();
        loop {
            let container = NSTextContainer::initWithSize(NSTextContainer::alloc(), inner);
            container.setLineFragmentPadding(0.0);
            layout.addTextContainer(&container);
            let range = layout.glyphRangeForTextContainer(&container);
            let done = range.location + range.length >= layout.numberOfGlyphs();
            let empty = range.length == 0;
            containers.push(container);
            // A page that holds nothing while glyphs remain would loop forever; stop with what there is.
            if done || empty || containers.len() > 2000 {
                break;
            }
        }
        let writer = PdfWriter::create(output)?;
        for container in &containers {
            let range: NSRange = layout.glyphRangeForTextContainer(container);
            writer.begin(PAGE);
            CGContext::save_g_state(Some(&writer.context));
            // Text is laid out from the top; flip the page so that its origin is the top left corner.
            CGContext::translate_ctm(Some(&writer.context), 0.0, PAGE.height);
            CGContext::scale_ctm(Some(&writer.context), 1.0, -1.0);
            let graphics =
                NSGraphicsContext::graphicsContextWithCGContext_flipped(&writer.context, true);
            NSGraphicsContext::saveGraphicsState_class();
            NSGraphicsContext::setCurrentContext(Some(&graphics));
            let origin = objc2_foundation::NSPoint::new(MARGIN, MARGIN);
            layout.drawBackgroundForGlyphRange_atPoint(range, origin);
            layout.drawGlyphsForGlyphRange_atPoint(range, origin);
            NSGraphicsContext::restoreGraphicsState_class();
            CGContext::restore_g_state(Some(&writer.context));
            writer.end();
        }
        writer.finish();
        Ok(())
    })
}

// -- publishing and probing ------------------------------------------------------------------------------------

/// Renames `from` to `to` only if nothing is at `to`: the check and the rename are one step of the file system,
/// so a file or a folder (even an empty one) that appears there in the meantime is never replaced. The error
/// has the kind `AlreadyExists` when `to` is taken.
pub fn rename_exclusive(from: &Path, to: &Path) -> std::io::Result<()> {
    use std::{ffi::CString, os::unix::ffi::OsStrExt};
    let c_path = |p: &Path| {
        CString::new(p.as_os_str().as_bytes())
            .map_err(|_| std::io::Error::from(std::io::ErrorKind::InvalidInput))
    };
    let (from, to) = (c_path(from)?, c_path(to)?);
    // SAFETY: both are NUL-terminated strings that live through the call.
    let rc = unsafe { libc::renamex_np(from.as_ptr(), to.as_ptr(), libc::RENAME_EXCL) };
    if rc == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

/// Whether a movie or sound file has at least one audio track. Only the header is read. A file that cannot be
/// read has none.
pub fn has_audio_track(path: &Path) -> bool {
    use objc2_av_foundation::{AVMediaTypeAudio, AVURLAsset};
    autoreleasepool(|_| {
        let url = file_url(path);
        // SAFETY: the URL is valid; the media type is a process-lifetime constant. The synchronous track list is
        // deprecated but, for a local file, reads only the header, and the wheel asks while it opens.
        unsafe {
            let asset = AVURLAsset::URLAssetWithURL_options(&url, None);
            let Some(kind) = AVMediaTypeAudio else {
                return false;
            };
            #[allow(deprecated)]
            let tracks = asset.tracksWithMediaType(kind);
            !tracks.is_empty()
        }
    })
}

// -- pasteboard, AirDrop, Terminal ---------------------------------------------------------------------------------

/// Replaces the contents of `pasteboard` with these files, as Finder's Copy does: they paste as files in
/// Finder and as paths in a terminal.
pub fn write_file_urls(pasteboard: &NSPasteboard, paths: &[PathBuf]) -> bool {
    autoreleasepool(|_| {
        let urls: Vec<Retained<NSURL>> = paths.iter().map(|p| file_url(p)).collect();
        let items: Vec<Retained<ProtocolObject<dyn NSPasteboardWriting>>> = urls
            .iter()
            .map(|u| ProtocolObject::from_retained(u.clone()))
            .collect();
        let array = NSArray::from_retained_slice(&items);
        pasteboard.clearContents();
        pasteboard.writeObjects(&array)
    })
}

/// Puts files on the general pasteboard.
pub fn copy_files(paths: &[PathBuf]) -> bool {
    !paths.is_empty() && write_file_urls(&NSPasteboard::generalPasteboard(), paths)
}

/// Hands the files to AirDrop's sharing sheet. The sheet shows by itself; this returns at once.
pub fn airdrop(_mtm: MainThreadMarker, paths: &[PathBuf]) -> Result<(), String> {
    // SAFETY: the name is a process-lifetime constant.
    let service =
        NSSharingService::sharingServiceNamed(unsafe { NSSharingServiceNameSendViaAirDrop })
            .ok_or("AirDrop is not available on this Mac")?;
    let urls: Vec<Retained<NSURL>> = paths.iter().map(|p| file_url(p)).collect();
    let items: Retained<NSArray<NSURL>> = NSArray::from_retained_slice(&urls);
    // SAFETY: the items are file URLs, which AirDrop takes.
    unsafe {
        let items = &*(&*items as *const NSArray<NSURL> as *const NSArray);
        if !service.canPerformWithItems(Some(items)) {
            return Err("AirDrop cannot send these files".into());
        }
        service.performWithItems(items);
    }
    Ok(())
}

/// Opens a Terminal window in `dir`. Refuses anything that is not a folder.
pub fn open_terminal(dir: &Path) -> Result<(), String> {
    if !dir.is_dir() {
        return Err("Terminal opens at a folder".into());
    }
    // `open` hands the folder to Terminal, which starts its shell there. The path is one argument and
    // is absolute, so it cannot be read as an option.
    let dir = fs::canonicalize(dir).map_err(|e| e.to_string())?;
    Command::new("/usr/bin/open")
        .args(["-b", "com.apple.Terminal"])
        .arg(dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("cannot start open: {e}"))
}

#[cfg(test)]
mod guard {
    /// A folder under the system temp folder that is removed when it goes out of scope, so also when an assertion
    /// fails and the test stops early.
    pub struct TempDir(std::path::PathBuf);

    impl TempDir {
        pub fn new(tag: &str) -> Self {
            static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
            let dir = std::env::temp_dir().join(format!(
                "{tag}-{}-{}",
                std::process::id(),
                N.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }
    }

    impl std::ops::Deref for TempDir {
        type Target = std::path::Path;
        fn deref(&self) -> &std::path::Path {
            &self.0
        }
    }

    impl AsRef<std::path::Path> for TempDir {
        fn as_ref(&self) -> &std::path::Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::guard::TempDir;
    use super::*;

    fn temp_dir() -> TempDir {
        TempDir::new("figghost-file-ops")
    }

    fn dictionary(
        entries: &[(&NSString, &AnyObject)],
    ) -> Retained<NSMutableDictionary<NSString, AnyObject>> {
        let d: Retained<NSMutableDictionary<NSString, AnyObject>> = NSMutableDictionary::new();
        for (k, v) in entries {
            // SAFETY: plain Foundation values.
            unsafe { d.setObject_forKey(*v, ProtocolObject::from_ref(*k)) };
        }
        d
    }

    /// A picture of `w` by `h` pixels in the format `uti`, carrying GPS, Exif and TIFF tags and an
    /// orientation, written by ImageIO itself.
    fn tagged_image(path: &Path, uti: &str, w: usize, h: usize) {
        let space = CGColorSpace::new_device_rgb().unwrap();
        // SAFETY: null data lets CoreGraphics own the memory.
        let context = unsafe {
            CGBitmapContextCreate(
                ptr::null_mut(),
                w,
                h,
                8,
                0,
                Some(&space),
                CGImageAlphaInfo::PremultipliedLast.0,
            )
        }
        .unwrap();
        CGContext::set_rgb_fill_color(Some(&context), 0.9, 0.3, 0.1, 1.0);
        CGContext::fill_rect(
            Some(&context),
            CGRect {
                origin: CGPoint { x: 0.0, y: 0.0 },
                size: CGSize {
                    width: w as f64 / 2.0,
                    height: h as f64,
                },
            },
        );
        let image = CGBitmapContextCreateImage(Some(&context)).unwrap();
        let gps = dictionary(&[
            (&NSString::from_str("Latitude"), &NSNumber::new_f64(21.0278)),
            (&NSString::from_str("LatitudeRef"), &NSString::from_str("N")),
            (
                &NSString::from_str("Longitude"),
                &NSNumber::new_f64(105.8342),
            ),
            (
                &NSString::from_str("LongitudeRef"),
                &NSString::from_str("E"),
            ),
        ]);
        let exif = dictionary(&[(
            &NSString::from_str("UserComment"),
            &NSString::from_str("secret comment"),
        )]);
        let tiff = dictionary(&[
            (
                &NSString::from_str("Make"),
                &NSString::from_str("SecretMaker"),
            ),
            (
                &NSString::from_str("Model"),
                &NSString::from_str("SecretModel"),
            ),
        ]);
        let properties = dictionary(&[
            (&NSString::from_str("{GPS}"), &gps),
            (&NSString::from_str("{Exif}"), &exif),
            (&NSString::from_str("{TIFF}"), &tiff),
            (&NSString::from_str("Orientation"), &NSNumber::new_i32(6)),
        ]);
        let url = file_url(path);
        // SAFETY: valid URL and type; the properties are plain dictionaries.
        unsafe {
            let destination = CGImageDestination::with_url(
                cf_url(&url),
                cf_string(&NSString::from_str(uti)),
                1,
                None,
            )
            .unwrap();
            destination.add_image(&image, Some(cf_dictionary(&properties)));
            assert!(destination.finalize());
        }
    }

    fn sub(
        properties: &NSDictionary<NSString, AnyObject>,
        key: &str,
        name: &str,
    ) -> Option<Retained<AnyObject>> {
        let inner = properties.objectForKey(&NSString::from_str(key))?;
        let inner = inner.downcast_ref::<NSDictionary>()?;
        let name = NSString::from_str(name);
        inner.objectForKey(name.as_ref())
    }

    fn number(properties: &NSDictionary<NSString, AnyObject>, key: &str) -> Option<i64> {
        let value = properties.objectForKey(&NSString::from_str(key))?;
        Some(value.downcast_ref::<NSNumber>()?.longLongValue())
    }

    /// For each format the wheel offers "Strip metadata" on: the tags go, the orientation and the size stay.
    #[test]
    fn stripping_removes_gps_exif_and_tiff_tags_and_keeps_the_orientation_and_size() {
        let dir = temp_dir();
        for (uti, ext) in [
            ("public.jpeg", "jpg"),
            ("public.png", "png"),
            ("public.tiff", "tiff"),
            ("public.heic", "heic"),
        ] {
            let source = dir.join(format!("tagged.{ext}"));
            tagged_image(&source, uti, 64, 48);
            let before = image_properties(&source).unwrap();
            if ext == "jpg" || ext == "heic" {
                assert!(
                    sub(&before, "{GPS}", "Latitude").is_some(),
                    "{ext}: the fixture should carry GPS"
                );
            }
            let output = dir.join(format!("clean.{ext}"));
            strip_image_metadata(&source, &output).unwrap_or_else(|e| panic!("{ext}: {e}"));
            let after = image_properties(&output).unwrap_or_else(|| panic!("{ext}: unreadable"));
            assert!(
                sub(&after, "{GPS}", "Latitude").is_none(),
                "{ext}: GPS left"
            );
            assert!(
                sub(&after, "{Exif}", "UserComment").is_none(),
                "{ext}: the Exif comment is left"
            );
            assert!(
                sub(&after, "{TIFF}", "Make").is_none(),
                "{ext}: the TIFF make is left"
            );
            assert_eq!(number(&after, "PixelWidth"), Some(64), "{ext}");
            assert_eq!(number(&after, "PixelHeight"), Some(48), "{ext}");
            assert_eq!(number(&after, "Orientation"), Some(6), "{ext}: orientation");
        }
    }

    #[test]
    fn stripping_something_that_is_not_an_image_fails() {
        let dir = temp_dir();
        let fake = dir.join("fake.jpg");
        fs::write(&fake, "not a picture").unwrap();
        assert!(strip_image_metadata(&fake, &dir.join("out.jpg")).is_err());
    }

    /// A PDF with one page of each size, each filled with a colour.
    fn make_pdf(path: &Path, sizes: &[(f64, f64)]) {
        let writer = PdfWriter::create(path).unwrap();
        for (i, (w, h)) in sizes.iter().enumerate() {
            writer.begin(CGSize {
                width: *w,
                height: *h,
            });
            CGContext::set_rgb_fill_color(
                Some(&writer.context),
                0.1 * (i + 1) as f64,
                0.2,
                0.9,
                1.0,
            );
            CGContext::fill_rect(
                Some(&writer.context),
                CGRect {
                    origin: CGPoint { x: 0.0, y: 0.0 },
                    size: CGSize {
                        width: *w / 2.0,
                        height: *h,
                    },
                },
            );
            writer.end();
        }
        writer.finish();
    }

    #[test]
    fn merging_keeps_every_page_in_the_order_given() {
        let dir = temp_dir();
        let (a, b, out) = (dir.join("a.pdf"), dir.join("b.pdf"), dir.join("m.pdf"));
        make_pdf(&a, &[(200.0, 100.0), (300.0, 100.0)]);
        make_pdf(&b, &[(400.0, 100.0)]);
        pdf_merge(&[b.clone(), a.clone()], &out).unwrap();
        assert_eq!(pdf_page_count(&out).unwrap(), 3);
        let document = open_pdf(&out).unwrap();
        let widths: Vec<f64> = (1..=3)
            .map(|n| {
                page_rect(&CGPDFDocument::page(Some(&document), n).unwrap())
                    .size
                    .width
            })
            .collect();
        assert_eq!(widths, [400.0, 200.0, 300.0]);
        assert!(
            pdf_merge(std::slice::from_ref(&a), &dir.join("one.pdf")).is_err(),
            "one file is not a merge"
        );
        assert!(pdf_merge(&[b, dir.join("gone.pdf")], &dir.join("x.pdf")).is_err());
        assert!(
            !dir.join("x.pdf").exists(),
            "nothing is written when a source fails"
        );
    }

    #[test]
    fn splitting_writes_one_pdf_per_page() {
        let dir = temp_dir();
        let source = dir.join("doc.pdf");
        make_pdf(&source, &[(200.0, 100.0), (300.0, 100.0), (400.0, 100.0)]);
        let out = dir.join("doc split");
        assert_eq!(pdf_split(&source, &out, "doc").unwrap(), 3);
        let mut names: Vec<String> = fs::read_dir(&out)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        assert_eq!(names, ["doc 1.pdf", "doc 2.pdf", "doc 3.pdf"]);
        let second = open_pdf(&out.join("doc 2.pdf")).unwrap();
        assert_eq!(CGPDFDocument::number_of_pages(Some(&second)), 1);
        assert_eq!(
            page_rect(&CGPDFDocument::page(Some(&second), 1).unwrap())
                .size
                .width,
            300.0
        );
        assert!(
            pdf_split(&source, &out, "doc").is_err(),
            "an existing folder is never reused"
        );
    }

    #[test]
    fn pages_become_pictures_at_300_dpi() {
        let dir = temp_dir();
        let source = dir.join("letter.pdf");
        make_pdf(&source, &[(612.0, 792.0), (72.0, 36.0)]);
        for (format, ext) in [(PageImage::Png, "png"), (PageImage::Jpeg, "jpg")] {
            let out = dir.join(format!("pages {ext}"));
            assert_eq!(pdf_to_images(&source, &out, "letter", format).unwrap(), 2);
            let first = image_properties(&out.join(format!("letter 1.{ext}"))).unwrap();
            assert_eq!(number(&first, "PixelWidth"), Some(2550), "{ext}");
            assert_eq!(number(&first, "PixelHeight"), Some(3300), "{ext}");
            assert_eq!(number(&first, "DPIWidth"), Some(300), "{ext}");
            let second = image_properties(&out.join(format!("letter 2.{ext}"))).unwrap();
            assert_eq!(number(&second, "PixelWidth"), Some(300), "{ext}");
            assert_eq!(number(&second, "PixelHeight"), Some(150), "{ext}");
        }
    }

    #[test]
    fn a_pdf_that_is_not_one_is_refused_without_creating_anything() {
        let dir = temp_dir();
        let fake = dir.join("fake.pdf");
        fs::write(&fake, "not a pdf").unwrap();
        assert!(pdf_to_images(&fake, &dir.join("pages"), "fake", PageImage::Png).is_err());
        assert!(!dir.join("pages").exists());
        assert!(pdf_split(&fake, &dir.join("split"), "fake").is_err());
        assert!(!dir.join("split").exists());
    }

    #[test]
    fn a_long_rich_text_document_prints_on_several_pages() {
        let dir = temp_dir();
        let source = dir.join("long.rtf");
        let mut rtf = String::from(r"{\rtf1\ansi\deff0 {\fonttbl{\f0 Helvetica;}}\f0\fs24 ");
        for n in 0..120 {
            rtf.push_str(&format!(
                r"Paragraph {n} of the long document, with some {{\b bold}} words.\par "
            ));
        }
        rtf.push('}');
        fs::write(&source, rtf).unwrap();
        let out = dir.join("long.pdf");
        rtf_to_pdf(&source, &out).unwrap();
        let pages = pdf_page_count(&out).unwrap();
        assert!(pages >= 2, "{pages} pages");
        let short = dir.join("short.rtf");
        fs::write(&short, r"{\rtf1\ansi Hello\par}").unwrap();
        rtf_to_pdf(&short, &dir.join("short.pdf")).unwrap();
        assert_eq!(pdf_page_count(&dir.join("short.pdf")).unwrap(), 1);
        fs::write(&short, "plain text, not RTF").unwrap();
        assert!(rtf_to_pdf(&short, &dir.join("bad.pdf")).is_err());
    }

    #[test]
    fn files_written_to_a_pasteboard_read_back_as_the_same_paths() {
        let dir = temp_dir();
        let (a, b) = (dir.join("one file.txt"), dir.join("two.txt"));
        fs::write(&a, "a").unwrap();
        fs::write(&b, "b").unwrap();
        let board = NSPasteboard::pasteboardWithUniqueName();
        assert!(write_file_urls(&board, &[a.clone(), b.clone()]));
        let paths = crate::pasteboard::file_paths(&board);
        let canon = |p: &Path| fs::canonicalize(p).unwrap();
        assert_eq!(
            paths
                .iter()
                .map(|p| canon(Path::new(p)))
                .collect::<Vec<_>>(),
            [canon(&a), canon(&b)]
        );
    }

    #[test]
    fn terminal_opens_only_at_a_folder() {
        let dir = temp_dir();
        let file = dir.join("f.txt");
        fs::write(&file, "x").unwrap();
        assert!(open_terminal(&file).is_err());
        assert!(open_terminal(&dir.join("gone")).is_err());
    }

    #[test]
    fn a_rename_never_replaces_what_is_already_there() {
        let dir = temp_dir();
        let (a, b) = (dir.join("a.txt"), dir.join("b.txt"));
        fs::write(&a, "a").unwrap();
        fs::write(&b, "b").unwrap();
        let err = rename_exclusive(&a, &b).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::AlreadyExists);
        assert_eq!(fs::read_to_string(&b).unwrap(), "b");
        assert!(a.exists(), "the source stays when the target is taken");
        // A free name works.
        let c = dir.join("c.txt");
        rename_exclusive(&a, &c).unwrap();
        assert!(!a.exists() && fs::read_to_string(&c).unwrap() == "a");
        // An empty folder is taken too, which a plain rename would replace.
        let (from, to) = (dir.join("from"), dir.join("to"));
        fs::create_dir(&from).unwrap();
        fs::write(from.join("f"), "f").unwrap();
        fs::create_dir(&to).unwrap();
        assert_eq!(
            rename_exclusive(&from, &to).unwrap_err().kind(),
            std::io::ErrorKind::AlreadyExists
        );
        assert!(from.join("f").exists() && fs::read_dir(&to).unwrap().count() == 0);
        fs::remove_dir(&to).unwrap();
        rename_exclusive(&from, &to).unwrap();
        assert!(to.join("f").exists());
        assert_eq!(
            rename_exclusive(&dir.join("gone"), &dir.join("x"))
                .unwrap_err()
                .kind(),
            std::io::ErrorKind::NotFound
        );
    }

    #[test]
    fn a_movie_is_known_to_have_sound_only_when_it_has_an_audio_track() {
        let dir = temp_dir();
        let (silent, sound) = (dir.join("silent.mov"), dir.join("sound.mov"));
        fs::write(&silent, include_bytes!("../testdata/silent.mov")).unwrap();
        fs::write(&sound, include_bytes!("../testdata/sound.mov")).unwrap();
        assert!(!has_audio_track(&silent));
        assert!(has_audio_track(&sound));
        assert!(!has_audio_track(&dir.join("gone.mov")));
        let fake = dir.join("fake.mov");
        fs::write(&fake, "not a movie").unwrap();
        assert!(!has_audio_track(&fake));
        // A sound file has an audio track too.
        let song = Path::new("/System/Library/Sounds/Glass.aiff");
        if song.exists() {
            assert!(has_audio_track(song));
        }
    }
}
