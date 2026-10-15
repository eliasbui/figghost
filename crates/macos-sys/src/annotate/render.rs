//! Drawing the document: the one routine that paints the picture and its objects, used for the window and
//! for the saved file alike, and the conversions between PNG bytes and pixels.

use core_annotate::{
    Colour, Document, Kind, Object, PixelRect, Pixels, Point as P, Rect as R, pixelate,
    shape::{HIGHLIGHT_ALPHA, arrow_head},
};
use objc2::{
    AnyThread,
    rc::Retained,
    runtime::{AnyObject, ProtocolObject},
};
use objc2_app_kit::{
    NSBezierPath, NSBitmapImageFileType, NSBitmapImageRep, NSColor, NSCompositingOperation, NSFont,
    NSFontAttributeName, NSForegroundColorAttributeName, NSGraphicsContext, NSImage,
    NSImageInterpolation, NSLineBreakMode, NSLineCapStyle, NSLineJoinStyle,
    NSMutableParagraphStyle, NSParagraphStyleAttributeName, NSStringDrawing,
    NSStringDrawingOptions, NSStringNSExtendedStringDrawing, NSTextAlignment,
};
use objc2_core_foundation::CFRetained;
use objc2_core_graphics::{
    CGBitmapContextCreate, CGBitmapContextCreateImage, CGBitmapContextGetBytesPerRow,
    CGBitmapContextGetData, CGBlendMode, CGColorSpace, CGColorSpaceModel, CGContext, CGImage,
    CGImageAlphaInfo, kCGColorSpaceSRGB,
};
use objc2_foundation::{
    NSData, NSDictionary, NSMutableDictionary, NSPoint, NSRect, NSSize, NSString,
};
use std::{cell::RefCell, collections::HashMap, ptr};

use crate::style::{Weight, font};

/// The most pixels a picture may have for the editor to open it.
const MAX_PIXELS: usize = 120_000_000;
const BLUR_CACHE: usize = 12;

/// The picture being annotated, kept in the colour space it came in so that drawing it back changes nothing.
pub struct Source {
    image: Retained<NSImage>,
    space: CFRetained<CGColorSpace>,
    pixels: Pixels,
    blurs: RefCell<HashMap<(PixelRect, u32), Retained<NSImage>>>,
}

fn rgba_info() -> u32 {
    CGImageAlphaInfo::PremultipliedLast.0
}

fn bitmap_context(
    width: usize,
    height: usize,
    space: &CGColorSpace,
) -> Option<CFRetained<CGContext>> {
    // SAFETY: a null data pointer asks CoreGraphics to allocate the (zeroed) bitmap itself.
    unsafe {
        CGBitmapContextCreate(
            ptr::null_mut(),
            width,
            height,
            8,
            0,
            Some(space),
            rgba_info(),
        )
    }
}

impl Source {
    /// `None` when the bytes are not a picture, or one too large to edit.
    pub fn from_png(png: &[u8]) -> Option<Self> {
        let rep = NSBitmapImageRep::imageRepWithData(&NSData::with_bytes(png))?;
        let cg = rep.CGImage()?;
        let (w, h) = (CGImage::width(Some(&cg)), CGImage::height(Some(&cg)));
        if w == 0 || h == 0 || w.checked_mul(h)? > MAX_PIXELS {
            return None;
        }
        let space = CGImage::color_space(Some(&cg))
            .filter(|s| CGColorSpace::model(Some(s)) == CGColorSpaceModel::RGB)
            // SAFETY: an extern static constant that lives as long as the process.
            .or_else(|| CGColorSpace::with_name(Some(unsafe { kCGColorSpaceSRGB })))?;
        let ctx = bitmap_context(w, h, &space)?;
        CGContext::set_blend_mode(Some(&ctx), CGBlendMode::Copy);
        CGContext::draw_image(
            Some(&ctx),
            NSRect::new(NSPoint::ZERO, NSSize::new(w as f64, h as f64)),
            Some(&cg),
        );
        let pixels = read_pixels(&ctx, w, h)?;
        let image =
            NSImage::initWithCGImage_size(NSImage::alloc(), &cg, NSSize::new(w as f64, h as f64));
        Some(Self {
            image,
            space,
            pixels,
            blurs: RefCell::default(),
        })
    }

    /// The name of the colour space the picture is kept in, as the export will embed it.
    pub fn colour_space_name(&self) -> String {
        CGColorSpace::name(Some(&self.space))
            .map(|name| name.to_string())
            .unwrap_or_default()
    }

    pub fn size_pixels(&self) -> (u32, u32) {
        (self.pixels.width, self.pixels.height)
    }

    pub fn pixels(&self) -> &Pixels {
        &self.pixels
    }

    /// The redacted pixels of a blur region as an image, from the original picture and nothing else.
    fn blurred(&self, region: PixelRect, block: u32) -> Option<Retained<NSImage>> {
        if let Some(found) = self.blurs.borrow().get(&(region, block)) {
            return Some(found.clone());
        }
        let out = pixelate(&self.pixels, region, block);
        let (w, h) = (out.width as usize, out.height as usize);
        let ctx = bitmap_context(w, h, &self.space)?;
        write_pixels(&ctx, &out)?;
        let cg = CGBitmapContextCreateImage(Some(&ctx))?;
        let image =
            NSImage::initWithCGImage_size(NSImage::alloc(), &cg, NSSize::new(w as f64, h as f64));
        let mut cache = self.blurs.borrow_mut();
        if cache.len() >= BLUR_CACHE {
            cache.clear();
        }
        cache.insert((region, block), image.clone());
        Some(image)
    }
}

fn read_pixels(ctx: &CGContext, w: usize, h: usize) -> Option<Pixels> {
    let data = CGBitmapContextGetData(Some(ctx)).cast::<u8>();
    let stride = CGBitmapContextGetBytesPerRow(Some(ctx));
    if data.is_null() || stride < w * 4 {
        return None;
    }
    let mut out = Vec::with_capacity(w * h * 4);
    for row in 0..h {
        // SAFETY: the context owns h rows of `stride` bytes at `data`, and nothing else touches it here.
        out.extend_from_slice(unsafe { std::slice::from_raw_parts(data.add(row * stride), w * 4) });
    }
    Pixels::new(w as u32, h as u32, out)
}

fn write_pixels(ctx: &CGContext, pixels: &Pixels) -> Option<()> {
    let (w, h) = (pixels.width as usize, pixels.height as usize);
    let data = CGBitmapContextGetData(Some(ctx)).cast::<u8>();
    let stride = CGBitmapContextGetBytesPerRow(Some(ctx));
    if data.is_null() || stride < w * 4 {
        return None;
    }
    for row in 0..h {
        let line = &pixels.data[row * w * 4..(row + 1) * w * 4];
        // SAFETY: the context owns h rows of `stride` bytes at `data`; `line` is exactly one row of pixels.
        unsafe { ptr::copy_nonoverlapping(line.as_ptr(), data.add(row * stride), w * 4) };
    }
    Some(())
}

pub fn ns_rect(r: R) -> NSRect {
    NSRect::new(NSPoint::new(r.x, r.y), NSSize::new(r.w, r.h))
}

pub fn ns_point(p: P) -> NSPoint {
    NSPoint::new(p.x, p.y)
}

pub fn ns_colour(c: Colour, alpha: f64) -> Retained<NSColor> {
    NSColor::colorWithSRGBRed_green_blue_alpha(
        f64::from(c.r) / 255.0,
        f64::from(c.g) / 255.0,
        f64::from(c.b) / 255.0,
        f64::from(c.a) / 255.0 * alpha,
    )
}

pub fn text_font(size: f64) -> Retained<NSFont> {
    font(size, Weight::Semibold)
}

fn text_attributes(
    size: f64,
    colour: &NSColor,
) -> Retained<NSMutableDictionary<NSString, AnyObject>> {
    let style = NSMutableParagraphStyle::new();
    style.setLineBreakMode(NSLineBreakMode::ByWordWrapping);
    style.setAlignment(NSTextAlignment::Left);
    let dict = NSMutableDictionary::<NSString, AnyObject>::new();
    let font = text_font(size);
    // SAFETY: extern statics are constants; the values are valid attribute objects for those keys.
    unsafe {
        dict.setObject_forKey(&*font, ProtocolObject::from_ref(NSFontAttributeName));
        dict.setObject_forKey(
            colour,
            ProtocolObject::from_ref(NSForegroundColorAttributeName),
        );
        dict.setObject_forKey(
            &*style,
            ProtocolObject::from_ref(NSParagraphStyleAttributeName),
        );
    }
    dict
}

/// Draws `text` wrapped in `rect` as an object of that size would show it.
pub fn draw_text_block(text: &str, size: f64, colour: &NSColor, rect: R) {
    let attrs = text_attributes(size, colour);
    // SAFETY: called with a valid graphics context; `attrs` holds valid attribute objects.
    unsafe { NSString::from_str(text).drawInRect_withAttributes(ns_rect(rect), Some(&attrs)) };
}

/// The height `text` needs at `size` in a box `width` wide, as the same layout will draw it.
pub fn measure_text(text: &str, size: f64, width: f64) -> f64 {
    let attrs = text_attributes(size, &NSColor::blackColor());
    // SAFETY: the attributes dictionary holds valid attribute objects.
    let bounds = unsafe {
        NSString::from_str(text).boundingRectWithSize_options_attributes_context(
            NSSize::new(width.max(1.0), 1.0e6),
            NSStringDrawingOptions::UsesLineFragmentOrigin
                | NSStringDrawingOptions::UsesFontLeading,
            Some(&attrs),
            None,
        )
    };
    bounds.size.height.ceil()
}

/// Paints the document into the current graphics context, which must be flipped and set up so that one unit
/// is one point of the document. `exact` draws the picture without smoothing, for the saved file; `skip` is an
/// object that something else is showing (a text being edited).
pub fn paint_document(doc: &Document, source: &Source, exact: bool, skip: Option<u64>) {
    let Some(context) = NSGraphicsContext::currentContext() else {
        return;
    };
    NSGraphicsContext::saveGraphicsState_class();
    NSBezierPath::clipRect(ns_rect(doc.canvas()));
    let background = doc.background();
    if background.a > 0 {
        ns_colour(background, 1.0).setFill();
        objc2_app_kit::NSRectFill(ns_rect(doc.canvas()));
    }
    context.setImageInterpolation(if exact {
        NSImageInterpolation::None
    } else {
        NSImageInterpolation::High
    });
    // SAFETY: a valid image, rectangles and no hints, drawn into the current context.
    unsafe {
        source
            .image
            .drawInRect_fromRect_operation_fraction_respectFlipped_hints(
                ns_rect(doc.image_rect()),
                NSRect::ZERO,
                NSCompositingOperation::SourceOver,
                1.0,
                true,
                None,
            );
    }
    for object in doc.objects().iter().filter(|o| Some(o.id) != skip) {
        paint_object(doc, source, object, &context);
    }
    NSGraphicsContext::restoreGraphicsState_class();
}

fn path_through(points: &[P]) -> Retained<NSBezierPath> {
    let path = NSBezierPath::new();
    if let Some((first, rest)) = points.split_first() {
        path.moveToPoint(ns_point(*first));
        for p in rest {
            path.lineToPoint(ns_point(*p));
        }
    }
    path
}

fn paint_object(doc: &Document, source: &Source, object: &Object, context: &NSGraphicsContext) {
    let alpha = if matches!(object.kind, Kind::Highlight(_)) {
        HIGHLIGHT_ALPHA
    } else {
        1.0
    };
    let colour = ns_colour(object.style.colour, alpha);
    let width = object.stroke_width();
    let stroke = |path: &NSBezierPath, cap: NSLineCapStyle| {
        colour.setStroke();
        path.setLineWidth(width);
        path.setLineCapStyle(cap);
        path.setLineJoinStyle(NSLineJoinStyle::Round);
        path.stroke();
    };
    match &object.kind {
        Kind::Arrow { from, to } => {
            if let Some(head) = arrow_head(*from, *to, object.style.width) {
                stroke(
                    &path_through(&[*from, head.shaft_end]),
                    NSLineCapStyle::Butt,
                );
                let triangle = path_through(&[head.tip, head.left, head.right]);
                triangle.closePath();
                colour.setFill();
                triangle.fill();
            }
        }
        Kind::Line { from, to } => stroke(&path_through(&[*from, *to]), NSLineCapStyle::Round),
        Kind::Rectangle(r) => {
            let path = NSBezierPath::bezierPathWithRect(ns_rect(*r));
            if object.style.fill {
                colour.setFill();
                path.fill();
            }
            stroke(&path, NSLineCapStyle::Butt);
        }
        Kind::Ellipse(r) => {
            let path = NSBezierPath::bezierPathWithOvalInRect(ns_rect(*r));
            if object.style.fill {
                colour.setFill();
                path.fill();
            }
            stroke(&path, NSLineCapStyle::Butt);
        }
        Kind::Pen(points) | Kind::Highlight(points) => {
            let flat = matches!(object.kind, Kind::Highlight(_));
            if points.len() == 1 {
                colour.setFill();
                let r = width / 2.0;
                NSBezierPath::bezierPathWithOvalInRect(NSRect::new(
                    NSPoint::new(points[0].x - r, points[0].y - r),
                    NSSize::new(width, width),
                ))
                .fill();
            } else {
                let cap = if flat {
                    NSLineCapStyle::Butt
                } else {
                    NSLineCapStyle::Round
                };
                stroke(&path_through(points), cap);
            }
        }
        Kind::Text { rect, text } => {
            if !text.is_empty() {
                draw_text_block(text, object.style.font_size(), &colour, *rect);
            }
        }
        Kind::Blur(_) => {
            if let Some(job) = doc.blur_job(object)
                && let Some(image) = source.blurred(job.region, job.block)
            {
                let s = doc.scale();
                let dest = R::new(
                    job.region.x as f64 / s,
                    job.region.y as f64 / s,
                    f64::from(job.region.w) / s,
                    f64::from(job.region.h) / s,
                );
                // Copy, not draw over: the redaction replaces what is under it, other objects included.
                let before = context.imageInterpolation();
                context.setImageInterpolation(NSImageInterpolation::None);
                // SAFETY: as above.
                unsafe {
                    image.drawInRect_fromRect_operation_fraction_respectFlipped_hints(
                        ns_rect(dest),
                        NSRect::ZERO,
                        NSCompositingOperation::Copy,
                        1.0,
                        true,
                        None,
                    );
                }
                context.setImageInterpolation(before);
            }
        }
    }
}

/// The finished picture as PNG: the canvas at the pixel size of the source, in the source's colour space.
pub fn export_png(doc: &Document, source: &Source) -> Option<Vec<u8>> {
    let plan = doc.export_plan();
    let ctx = bitmap_context(plan.width as usize, plan.height as usize, &source.space)?;
    CGContext::translate_ctm(Some(&ctx), 0.0, f64::from(plan.height));
    CGContext::scale_ctm(Some(&ctx), plan.scale, -plan.scale);
    CGContext::translate_ctm(Some(&ctx), -plan.origin.x, -plan.origin.y);
    let graphics = NSGraphicsContext::graphicsContextWithCGContext_flipped(&ctx, true);
    NSGraphicsContext::saveGraphicsState_class();
    NSGraphicsContext::setCurrentContext(Some(&graphics));
    paint_document(doc, source, true, None);
    NSGraphicsContext::restoreGraphicsState_class();
    let image = CGBitmapContextCreateImage(Some(&ctx))?;
    let rep = NSBitmapImageRep::initWithCGImage(NSBitmapImageRep::alloc(), &image);
    // The size in points makes the file say how many pixels per point it has.
    rep.setSize(NSSize::new(
        f64::from(plan.width) / plan.scale,
        f64::from(plan.height) / plan.scale,
    ));
    // SAFETY: an empty properties dictionary is valid for PNG output.
    let png = unsafe {
        rep.representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new())
    }?;
    Some(png.to_vec())
}
