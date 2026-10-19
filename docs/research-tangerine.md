# What to take from Tangerine

Tangerine for Mac turns a file drag into a menu: while a file is being dragged, a key opens a wheel of choices
around the pointer, and dropping the file on one of them converts it, runs a tool on it or does something with it.
This page lists what it offers, what figghost already has, what can be built with the tools that ship with macOS,
and what cannot.

Sources: tangerineformac.com as read on 19 October 2026 (its demo says "All operations on this page are
simulated", so behaviour is what the page describes, not what was observed), and a summary of the app the user
supplied. Where the two disagree it is said below. The macOS column was checked on this Mac (macOS 27, Apple
silicon) on the same day.

## How it is used

| Gesture | What opens | Notes |
|---|---|---|
| Drag a file, tap Shift | The conversion wheel | The new file is written next to the original. |
| Drag a file, tap Command | The file tools wheel | The supplied summary says Shift and Option instead; the site says Command. figghost follows the site. |
| Drag a file or folder, tap Control | The actions wheel | Works with folders, mixed selections and files that have no conversion. |
| Return on a selected file | The format menu | Arrow keys choose, Return applies, Escape cancels, Command or Control switch wheels. |

The wheel stays open until the file is dropped. The keys can be changed in its settings. It runs from the menu bar,
on Apple silicon with macOS 13 or later, processes everything locally, and is sold on the Mac App Store with a free
allowance (the summary says 20 files) and a one-time purchase.

Finder gives the same keys a meaning during a drag of its own (Option copies, Command moves, both make an alias).
That does not get in the way: once the wheel is open the drop lands on the wheel, not in a Finder window.

## Formats and tools Tangerine lists

| Group | Outputs |
|---|---|
| Images | JPG, PNG, WebP, HEIC, TIFF, AVIF, BMP, PDF; SVG as input |
| Audio | MP3, M4A, WAV, FLAC, OGG, Opus, AIFF, WMA |
| Video | MP4, MOV, MKV, WebM, AVI, WMV, GIF, MP3 |
| PDF | DOCX, PAGES, PPTX, XLSX, JPG, PNG (all pages at 300 DPI), EPUB, TXT, MD |
| Documents | DOCX, PAGES, Markdown, TXT, EPUB, HTML; RTF, RTFD and text clippings as input |
| Slides and spreadsheets | PPT and PPTX to PDF, JPG, PNG; PDF to PPTX; XLSX, ODS, CSV, TSV, PDF |
| Subtitles | SRT, VTT, TXT |
| Archives | ZIP, TAR, GZIP; RAR as input |
| File tools (26 in all) | Compress, crop, trim, split, merge, "and more"; the summary adds removing metadata, redacting documents and annotating photos, and compression by a percentage for one file or a batch |
| Actions | AirDrop, Float (a file kept floating on screen to drag into another window later), Copy, Move, Copy Path, Terminal |

## Already in figghost

| Tangerine | Where it already is |
|---|---|
| Image to PNG, JPEG, HEIC | The shelf menu: `app-ui/src/shelf_actions.rs` through `sips`. |
| Smaller image | *Resize to half* and *Resize to a quarter* in the shelf menu. |
| ZIP | The shelf menu, through `ditto`. |
| Remove background, copy text from a picture | The shelf menu: `macos-sys/src/vision.rs`, `screen_capture.rs`. |
| Annotate a picture | The capture editor, `macos-sys/src/editor_ui.rs`, so far opened only after a capture. |
| Float | The notch shelf keeps dropped files and lets them be dragged out again. It lives in the notch, not on top of the screen. |
| Watching a drag | Window snapping watches the mouse with a global monitor (`snap_ui::MouseMonitor`), but only for window drags; nothing in the tree notices a file drag today. |

## Possible with what ships with macOS

Checked on this Mac. None of these needs a new dependency or a bundled engine.

| Group | Tool | What it can make |
|---|---|---|
| Images | `sips` | PNG, JPEG, HEIC, TIFF, AVIF, GIF, BMP, PDF, ICO (and PSD, JPEG 2000). WebP is read only: `sips --formats` lists it without "Writable". |
| Image metadata | ImageIO (`objc2-image-io` is already a dependency) | A copy without GPS, EXIF, TIFF and maker data. |
| Audio | `afconvert` | M4A and AAC, WAV, AIFF, CAF, FLAC. MP3 is decode only: encoding stops with `ExtAudioFileSetProperty ('cfmt') failed ('fmt?')`. |
| Video | `avconvert` | MOV, MP4 and M4V, the sound alone as M4A, smaller copies at 1080p, 720p and 540p, HEVC, and trimming (`--start`, `--duration`). |
| Documents | `textutil`, `cupsfilter` | TXT, HTML, RTF, RTFD, DOC, DOCX, ODT, WebArchive. PDF through the print system from plain text; the same command gave an empty file for RTF, so PDF from formatted documents needs another way (printing an `NSAttributedString` from AppKit). |
| PDF | CoreGraphics | Merging, splitting into pages, every page as a picture. |
| Archives | `ditto`, `tar` | ZIP, TAR, TAR with gzip. |
| Actions | AppKit | AirDrop (`NSSharingService`), the path or the files on the pasteboard, Terminal at a folder, Reveal in Finder, the notch shelf for Float. |

## Not possible without a bundled engine

| Tangerine | Why not |
|---|---|
| MP3, OGG, Opus, WMA out; MKV, WebM, AVI, WMV | No encoder or container in macOS. Needs ffmpeg, about 80 MB and an LGPL or GPL licence to honour. |
| WebP out | ImageIO reads WebP but has no writer. |
| PAGES, PPTX, XLSX, ODS, EPUB, Markdown; PDF to DOCX or PPTX | Needs LibreOffice or pandoc. pandoc is on this Mac only because Homebrew put it there; a user's Mac cannot be assumed to have it. |
| RAR in | No unpacker in macOS. |

The user chose built-in tools only for now, so these are not offered.

## Built this round

The drag wheel feature (`wheel`, off by default): by default Shift opens the formats, Command the tools and Control
the actions, with the lists above. Its settings page turns each wheel on or off, binds it to Shift, Command, Control
or Option (two wheels that are on never share a key) and hides the slots the user does not want. The wheel is a
Liquid Glass disc like the notch's glass material, with an opaque dark look under Increase Contrast or Reduce
Transparency and a dark vibrancy disc before macOS 26.

`macos-sys/src/wheel_ui.rs` watches the drag (a global mouse monitor and the drag pasteboard; the keys are read from
`NSEvent::modifierFlags()` while a file drag lasts, so no Accessibility access is needed) and draws the wheel;
`app-ui/src/features/wheel/` decides what each wheel offers and what each slot does. A job writes under a hidden
name next to the original and is published with a link or an exclusive rename, so a result never replaces a file
or folder that appeared under its name meanwhile; the shelf menu shares that runner.

Dropped after trying them on this Mac: ICO (`sips` writes it only for square images of icon sizes), resizing or
compressing WebP, stripping metadata from AVIF, GIF and BMP, and audio only from a video without sound.

## Later

| Feature | What is missing |
|---|---|
| Redacting a document | Drawing black boxes is easy; removing the text under them from the PDF is not, and a redaction that leaves the text behind is worse than none. Needs a PDF rewrite step and a test that extracts the text afterwards. |
| Trimming, cropping, splitting video | `avconvert` can trim, but the user has to say where: an input panel. |
| Move | Needs a destination. Tangerine does not say how it picks one. |
| The keyboard path | Return on a file in Finder means reading Finder's selection through Accessibility or Apple Events. |
| Float as a window on top of everything | A small panel that holds the file and can be dragged from, instead of the notch shelf. |
