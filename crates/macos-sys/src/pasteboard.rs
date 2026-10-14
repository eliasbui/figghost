use objc2::{rc::Retained, runtime::ProtocolObject};
use objc2_app_kit::{
    NSBitmapImageFileType, NSBitmapImageRep, NSPasteboard, NSPasteboardTypeFileURL,
    NSPasteboardTypeHTML, NSPasteboardTypePNG, NSPasteboardTypeRTF, NSPasteboardTypeString,
    NSPasteboardTypeTIFF, NSPasteboardWriting,
};
use objc2_foundation::{NSArray, NSData, NSDictionary, NSString, NSURL};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    pub png: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

/// What a pasteboard holds, in the priority the history uses: files, then text, then an image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Content {
    Files(Vec<String>),
    Text(String),
    Image(Image),
}

/// The formatted forms a text copy comes with.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Rich {
    pub rtf: Option<Vec<u8>>,
    pub html: Option<Vec<u8>>,
}

pub enum Write<'a> {
    Text(&'a str),
    /// Plain text with its formatting, so a rich text field takes it as it was copied.
    RichText {
        text: &'a str,
        rtf: Option<&'a [u8]>,
        html: Option<&'a [u8]>,
    },
    Png(&'a [u8]),
    Files(&'a [String]),
}

pub struct Pasteboard(Retained<NSPasteboard>);

impl Pasteboard {
    pub fn general() -> Self {
        Self(NSPasteboard::generalPasteboard())
    }

    /// A private named pasteboard, so tests never touch the user's clipboard.
    pub fn named(name: &str) -> Self {
        Self(NSPasteboard::pasteboardWithName(&NSString::from_str(name)))
    }

    /// Cheap enough to poll; it does not read any content.
    pub fn change_count(&self) -> isize {
        self.0.changeCount()
    }

    pub fn types(&self) -> Vec<String> {
        self.0
            .types()
            .map(|types| types.iter().map(|t| t.to_string()).collect())
            .unwrap_or_default()
    }

    pub fn read(&self) -> Option<Content> {
        let files = self.read_files();
        if !files.is_empty() {
            return Some(Content::Files(files));
        }
        // SAFETY: extern statics are process-lifetime constants.
        if let Some(text) = self.0.stringForType(unsafe { NSPasteboardTypeString }) {
            return Some(Content::Text(text.to_string()));
        }
        self.read_image().map(Content::Image)
    }

    fn read_files(&self) -> Vec<String> {
        let Some(items) = self.0.pasteboardItems() else {
            return Vec::new();
        };
        items
            .iter()
            .filter_map(|item| {
                // SAFETY: extern statics are process-lifetime constants.
                let url = item.stringForType(unsafe { NSPasteboardTypeFileURL })?;
                NSURL::URLWithString(&url)?.path().map(|p| p.to_string())
            })
            .collect()
    }

    fn read_image(&self) -> Option<Image> {
        // SAFETY: extern statics are process-lifetime constants.
        let (png, tiff) = unsafe { (NSPasteboardTypePNG, NSPasteboardTypeTIFF) };
        if let Some(data) = self.0.dataForType(png) {
            let rep = NSBitmapImageRep::imageRepWithData(&data)?;
            return Some(Image {
                png: data.to_vec(),
                width: rep.pixelsWide().max(0) as u32,
                height: rep.pixelsHigh().max(0) as u32,
            });
        }
        let data = self.0.dataForType(tiff)?;
        let rep = NSBitmapImageRep::imageRepWithData(&data)?;
        // SAFETY: an empty properties dictionary is valid for PNG output.
        let png = unsafe {
            rep.representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new())
        }?;
        Some(Image {
            png: png.to_vec(),
            width: rep.pixelsWide().max(0) as u32,
            height: rep.pixelsHigh().max(0) as u32,
        })
    }

    /// The RTF and HTML forms of the current content, when it has them.
    pub fn read_rich(&self) -> Option<Rich> {
        // SAFETY: extern statics are process-lifetime constants.
        let (rtf, html) = unsafe { (NSPasteboardTypeRTF, NSPasteboardTypeHTML) };
        let rich = Rich {
            rtf: self.0.dataForType(rtf).map(|d| d.to_vec()),
            html: self.0.dataForType(html).map(|d| d.to_vec()),
        };
        (rich.rtf.is_some() || rich.html.is_some()).then_some(rich)
    }

    /// Replaces the pasteboard content and returns its new change count, so the watcher can skip it.
    pub fn write(&self, content: Write<'_>) -> isize {
        self.0.clearContents();
        match content {
            Write::Text(text) => {
                // SAFETY: extern statics are process-lifetime constants.
                self.0.setString_forType(&NSString::from_str(text), unsafe {
                    NSPasteboardTypeString
                });
            }
            Write::RichText { text, rtf, html } => {
                // SAFETY: extern statics are process-lifetime constants.
                let (string, rtf_type, html_type) = unsafe {
                    (
                        NSPasteboardTypeString,
                        NSPasteboardTypeRTF,
                        NSPasteboardTypeHTML,
                    )
                };
                self.0.setString_forType(&NSString::from_str(text), string);
                if let Some(bytes) = rtf {
                    self.0
                        .setData_forType(Some(&NSData::with_bytes(bytes)), rtf_type);
                }
                if let Some(bytes) = html {
                    self.0
                        .setData_forType(Some(&NSData::with_bytes(bytes)), html_type);
                }
            }
            Write::Png(bytes) => {
                // SAFETY: as above.
                self.0
                    .setData_forType(Some(&NSData::with_bytes(bytes)), unsafe {
                        NSPasteboardTypePNG
                    });
            }
            Write::Files(paths) => {
                let urls: Vec<Retained<ProtocolObject<dyn NSPasteboardWriting>>> = paths
                    .iter()
                    .map(|p| {
                        ProtocolObject::from_retained(NSURL::fileURLWithPath(&NSString::from_str(
                            p,
                        )))
                    })
                    .collect();
                self.0.writeObjects(&NSArray::from_retained_slice(&urls));
            }
        }
        self.0.changeCount()
    }
}

/// File paths on any pasteboard, such as the one a drop delivers.
pub fn file_paths(pasteboard: &NSPasteboard) -> Vec<String> {
    let Some(items) = pasteboard.pasteboardItems() else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| {
            // SAFETY: extern statics are process-lifetime constants.
            let url = item.stringForType(unsafe { NSPasteboardTypeFileURL })?;
            NSURL::URLWithString(&url)?.path().map(|p| p.to_string())
        })
        .collect()
}
