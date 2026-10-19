//! The sound page of the notch: the output device and its software volume, the effects, and one row per
//! app with its volume, mute switch and output device. A right click on an app's name picks an
//! equalizer preset for that app alone.

use crate::{
    notch::Page,
    panel::Key,
    style::{self, Tone},
    widgets::{ActionTarget, PageRoot},
};
use objc2::{
    MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained, runtime::AnyObject, sel,
};
use objc2_app_kit::{
    NSBezelStyle, NSBorderType, NSButton, NSButtonType, NSColor, NSControlSize,
    NSControlStateValueOff, NSControlStateValueOn, NSFont, NSImage, NSImageView, NSLineBreakMode,
    NSMenu, NSMenuItem, NSPopUpButton, NSScrollView, NSScrollerStyle, NSSlider, NSTextAlignment,
    NSTextField, NSView, NSWorkspace,
};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};
use std::{cell::RefCell, collections::HashMap, rc::Rc};

const MARGIN: f64 = 16.0;
const FOOTER_HEIGHT: f64 = 24.0;
const TOP_ROWS: [f64; 7] = [22.0, 24.0, 26.0, 24.0, 24.0, 24.0, 24.0];
const ROW_HEIGHT: f64 = 34.0;
const ICON: f64 = 20.0;
const SMALL_FONT: f64 = 11.0;

/// Text the page shows; the app supplies it in the user's language.
#[derive(Debug, Clone, Default)]
pub struct MixerTexts {
    pub tab: String,
    pub hint: String,
    pub enabled: String,
    pub device_volume: String,
    pub boost: String,
    pub night: String,
    pub width: String,
    pub spatial: String,
    pub angle: String,
    pub orbit: String,
    pub mute: String,
    pub app_eq: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MixerRow {
    /// Bundle identifier.
    pub key: String,
    pub name: String,
    pub icon_path: Option<String>,
    pub playing: bool,
    /// Slider position, 0 to 1.
    pub volume: f64,
    pub percent: String,
    pub muted: bool,
    /// Index into `MixerModel::outputs`.
    pub output: usize,
    /// Index into `MixerModel::eq_options`.
    pub eq: usize,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct MixerModel {
    pub enabled: bool,
    /// Shown instead of the app list: nothing playing, taps unsupported, an error.
    pub notice: Option<String>,
    /// The default output, as a line of text.
    pub device_title: String,
    /// Slider position and percent text of the software volume, when the device has no volume of its own.
    pub device_volume: Option<(f64, String)>,
    /// First item follows the system's default output; then one per device.
    pub outputs: Vec<String>,
    /// Equalizer choices for the effects: the first turns it off.
    pub eq_options: Vec<String>,
    /// Equalizer choices for one app (its right-click menu).
    pub app_eq_options: Vec<String>,
    pub effects_eq: usize,
    /// 0 to 1.
    pub boost: f64,
    pub night: bool,
    /// Slider position, 0 to 1 (0.5 is unchanged).
    pub width: f64,
    /// 3D amount, 0 to 1.
    pub spatial: f64,
    /// Slider position, 0 to 1 (0.5 is straight ahead).
    pub angle: f64,
    /// Slider position, 0 to 1 (0.5 is not turning).
    pub orbit: f64,
    pub rows: Vec<MixerRow>,
}

/// A change to one app's row: its bundle identifier and the new value.
pub type RowChange<T> = Box<dyn Fn(&str, T)>;

pub struct MixerCallbacks {
    pub on_enabled: Box<dyn Fn(bool)>,
    pub on_device_volume: Box<dyn Fn(f64)>,
    pub on_effects_eq: Box<dyn Fn(usize)>,
    pub on_boost: Box<dyn Fn(f64)>,
    pub on_night: Box<dyn Fn(bool)>,
    pub on_width: Box<dyn Fn(f64)>,
    pub on_spatial: Box<dyn Fn(f64)>,
    pub on_angle: Box<dyn Fn(f64)>,
    pub on_orbit: Box<dyn Fn(f64)>,
    pub on_volume: RowChange<f64>,
    pub on_mute: RowChange<bool>,
    pub on_output: RowChange<usize>,
    pub on_eq: RowChange<usize>,
}

define_class!(
    // SAFETY: NSView allows subclassing; this one only changes the coordinate system.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigMixerList"]
    struct ListView;

    impl ListView {
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }
    }
);

impl ListView {
    fn new(mtm: MainThreadMarker, frame: NSRect) -> Retained<Self> {
        // SAFETY: designated initializer with valid arguments on the main thread.
        unsafe { msg_send![super(mtm.alloc::<Self>().set_ivars(())), initWithFrame: frame] }
    }
}

fn label(mtm: MainThreadMarker, text: &str, frame: NSRect, white: f64) -> Retained<NSTextField> {
    let label = NSTextField::labelWithString(&NSString::from_str(text), mtm);
    label.setFont(Some(&NSFont::systemFontOfSize(SMALL_FONT)));
    label.setTextColor(Some(&NSColor::colorWithWhite_alpha(1.0, white)));
    label.setLineBreakMode(NSLineBreakMode::ByTruncatingTail);
    label.setFrame(frame);
    label
}

fn on_off(on: bool) -> isize {
    if on {
        NSControlStateValueOn
    } else {
        NSControlStateValueOff
    }
}

fn target_object(target: &ActionTarget) -> &AnyObject {
    target
}

fn checkbox(
    mtm: MainThreadMarker,
    title: &str,
    frame: NSRect,
    target: &ActionTarget,
) -> Retained<NSButton> {
    // SAFETY: `fire:` takes the sender as its only argument; the caller keeps the target alive.
    let button = unsafe {
        NSButton::checkboxWithTitle_target_action(
            &NSString::from_str(title),
            Some(target_object(target)),
            Some(sel!(fire:)),
            mtm,
        )
    };
    button.setControlSize(NSControlSize::Small);
    button.setFont(Some(&NSFont::systemFontOfSize(SMALL_FONT)));
    button.setFrame(frame);
    button
}

fn slider(mtm: MainThreadMarker, frame: NSRect, target: &ActionTarget) -> Retained<NSSlider> {
    // SAFETY: as in `checkbox`.
    let slider = unsafe {
        NSSlider::sliderWithValue_minValue_maxValue_target_action(
            0.5,
            0.0,
            1.0,
            Some(target_object(target)),
            Some(sel!(fire:)),
            mtm,
        )
    };
    slider.setControlSize(NSControlSize::Small);
    slider.setContinuous(true);
    // White, as the notch draws its own controls, rather than the accent of a settings window.
    slider.setTrackFillColor(Some(&style::white(style::TEXT)));
    slider.setFrame(frame);
    slider
}

/// A speaker that an app's row is muted with: crossed out while muted. A symbol says it in less room than a
/// checkbox and a word, and `title` still names it for VoiceOver and in its tooltip.
fn mute_button(
    mtm: MainThreadMarker,
    title: &str,
    frame: NSRect,
    target: &ActionTarget,
) -> Retained<NSButton> {
    let symbol = |name: &str| {
        NSImage::imageWithSystemSymbolName_accessibilityDescription(
            &NSString::from_str(name),
            Some(&NSString::from_str(title)),
        )
    };
    let button = NSButton::initWithFrame(mtm.alloc(), frame);
    // SAFETY: as in `checkbox`.
    unsafe {
        button.setTarget(Some(target_object(target)));
        button.setAction(Some(sel!(fire:)));
    }
    // A toggle shows its alternate image while it is on, which is while the app is muted.
    button.setButtonType(NSButtonType::Toggle);
    button.setBordered(false);
    button.setImage(symbol("speaker.wave.2.fill").as_deref());
    button.setAlternateImage(symbol("speaker.slash.fill").as_deref());
    button.setToolTip(Some(&NSString::from_str(title)));
    button
}

fn popup(
    mtm: MainThreadMarker,
    frame: NSRect,
    items: &[String],
    target: &ActionTarget,
) -> Retained<NSPopUpButton> {
    let popup = NSPopUpButton::initWithFrame_pullsDown(mtm.alloc(), frame, false);
    popup.setControlSize(NSControlSize::Small);
    // The badge bezel (once called inline) is a small capsule, the shape of the chips on the other pages.
    popup.setBezelStyle(NSBezelStyle::Badge);
    popup.setFont(Some(&NSFont::systemFontOfSize(SMALL_FONT)));
    for item in items {
        popup.addItemWithTitle(&NSString::from_str(item));
    }
    // SAFETY: as in `checkbox`.
    unsafe {
        popup.setTarget(Some(target_object(target)));
        popup.setAction(Some(sel!(fire:)));
    }
    popup
}

fn set_items(popup: &NSPopUpButton, items: &[String]) {
    let current: Vec<String> = (0..popup.numberOfItems())
        .map(|i| popup.itemTitleAtIndex(i).to_string())
        .collect();
    if current != items {
        popup.removeAllItems();
        for item in items {
            popup.addItemWithTitle(&NSString::from_str(item));
        }
    }
}

fn selected(popup: &NSPopUpButton) -> usize {
    popup.indexOfSelectedItem().max(0) as usize
}

/// The controls of one app row, kept so values can be updated without rebuilding the row.
struct RowViews {
    key: String,
    view: Retained<NSView>,
    name: Retained<NSTextField>,
    slider: Retained<NSSlider>,
    percent: Retained<NSTextField>,
    mute: Retained<NSButton>,
    output: Retained<NSPopUpButton>,
    menu: Retained<NSMenu>,
    _targets: Vec<Retained<ActionTarget>>,
}

struct Header {
    enabled: Retained<NSButton>,
    device: Retained<NSTextField>,
    volume_label: Retained<NSTextField>,
    volume: Retained<NSSlider>,
    volume_percent: Retained<NSTextField>,
    eq: Retained<NSPopUpButton>,
    boost: Retained<NSSlider>,
    night: Retained<NSButton>,
    width: Retained<NSSlider>,
    spatial: Retained<NSSlider>,
    angle: Retained<NSSlider>,
    orbit: Retained<NSSlider>,
    _targets: Vec<Retained<ActionTarget>>,
}

pub struct MixerView {
    pub root: Retained<PageRoot>,
    mtm: MainThreadMarker,
    texts: MixerTexts,
    callbacks: Rc<MixerCallbacks>,
    header: RefCell<Option<Header>>,
    scroll: Retained<NSScrollView>,
    list: Retained<ListView>,
    notice: Retained<NSTextField>,
    rows: RefCell<Vec<RowViews>>,
    model: RefCell<MixerModel>,
    icons: RefCell<HashMap<String, Retained<NSImage>>>,
}

impl MixerView {
    pub fn new(
        mtm: MainThreadMarker,
        frame: NSRect,
        texts: MixerTexts,
        callbacks: MixerCallbacks,
    ) -> Rc<Self> {
        let root = PageRoot::new(mtm, frame, &texts.hint);
        let (width, height) = (frame.size.width, frame.size.height);
        let header_height: f64 = TOP_ROWS.iter().sum::<f64>() + 12.0;
        let list_top = height - header_height - 6.0;
        let scroll_frame = NSRect::new(
            NSPoint::new(0.0, FOOTER_HEIGHT),
            NSSize::new(width, list_top - FOOTER_HEIGHT),
        );
        let scroll = NSScrollView::initWithFrame(mtm.alloc(), scroll_frame);
        scroll.setHasVerticalScroller(true);
        scroll.setAutohidesScrollers(true);
        scroll.setScrollerStyle(NSScrollerStyle::Overlay);
        scroll.setDrawsBackground(false);
        scroll.setBorderType(NSBorderType::NoBorder);
        let list = ListView::new(mtm, NSRect::new(NSPoint::ZERO, scroll_frame.size));
        scroll.setDocumentView(Some(&list));
        root.addSubview(&scroll);

        let notice = label(
            mtm,
            "",
            NSRect::new(
                NSPoint::new(
                    MARGIN,
                    scroll_frame.origin.y + scroll_frame.size.height / 2.0,
                ),
                NSSize::new(width - 2.0 * MARGIN, 18.0),
            ),
            0.55,
        );
        notice.setAlignment(NSTextAlignment::Center);
        root.addSubview(&notice);

        let this = Rc::new(Self {
            root,
            mtm,
            texts,
            callbacks: Rc::new(callbacks),
            header: RefCell::new(None),
            scroll,
            list,
            notice,
            rows: RefCell::new(Vec::new()),
            model: RefCell::new(MixerModel::default()),
            icons: RefCell::new(HashMap::new()),
        });
        let header = this.build_header(width, height);
        *this.header.borrow_mut() = Some(header);
        this
    }

    fn build_header(&self, width: f64, height: f64) -> Header {
        let mtm = self.mtm;
        let texts = &self.texts;
        let mut targets = Vec::new();
        let mut y = height - 8.0;
        let mut row = |h: f64| {
            y -= h;
            y
        };

        let row1 = row(TOP_ROWS[0]);
        let enabled_target = {
            let callbacks = Rc::clone(&self.callbacks);
            let slot: Rc<RefCell<Option<Retained<NSButton>>>> = Rc::default();
            let reader = Rc::clone(&slot);
            let target = ActionTarget::new(mtm, move || {
                if let Some(button) = reader.borrow().as_ref() {
                    (callbacks.on_enabled)(button.state() == NSControlStateValueOn);
                }
            });
            (target, slot)
        };
        let enabled = checkbox(
            mtm,
            &texts.enabled,
            NSRect::new(NSPoint::new(MARGIN, row1), NSSize::new(170.0, TOP_ROWS[0])),
            &enabled_target.0,
        );
        *enabled_target.1.borrow_mut() = Some(enabled.clone());
        targets.push(enabled_target.0);
        let device = label(
            mtm,
            "",
            NSRect::new(
                NSPoint::new(190.0, row1 + 3.0),
                NSSize::new(width - 190.0 - MARGIN, 16.0),
            ),
            0.6,
        );
        device.setAlignment(NSTextAlignment::Right);

        let row2 = row(TOP_ROWS[1]);
        let volume_label = label(
            mtm,
            &texts.device_volume,
            NSRect::new(NSPoint::new(MARGIN, row2 + 4.0), NSSize::new(150.0, 16.0)),
            0.8,
        );
        let (volume, volume_target) = self.slider_with(
            NSRect::new(NSPoint::new(170.0, row2), NSSize::new(240.0, TOP_ROWS[1])),
            |c, v| (c.on_device_volume)(v),
        );
        targets.push(volume_target);
        let volume_percent = label(
            mtm,
            "",
            NSRect::new(NSPoint::new(416.0, row2 + 4.0), NSSize::new(68.0, 16.0)),
            0.8,
        );

        let row3 = row(TOP_ROWS[2]);
        let eq_target = {
            let callbacks = Rc::clone(&self.callbacks);
            let slot: Rc<RefCell<Option<Retained<NSPopUpButton>>>> = Rc::default();
            let reader = Rc::clone(&slot);
            (
                ActionTarget::new(mtm, move || {
                    if let Some(popup) = reader.borrow().as_ref() {
                        (callbacks.on_effects_eq)(selected(popup));
                    }
                }),
                slot,
            )
        };
        let eq = popup(
            mtm,
            NSRect::new(
                NSPoint::new(MARGIN - 4.0, row3),
                NSSize::new(158.0, TOP_ROWS[2]),
            ),
            &[],
            &eq_target.0,
        );
        *eq_target.1.borrow_mut() = Some(eq.clone());
        targets.push(eq_target.0);
        let boost_label = label(
            mtm,
            &texts.boost,
            NSRect::new(NSPoint::new(176.0, row3 + 5.0), NSSize::new(84.0, 16.0)),
            0.8,
        );
        let (boost, boost_target) = self.slider_with(
            NSRect::new(
                NSPoint::new(260.0, row3 + 1.0),
                NSSize::new(110.0, TOP_ROWS[2]),
            ),
            |c, v| (c.on_boost)(v),
        );
        targets.push(boost_target);
        let night_target = {
            let callbacks = Rc::clone(&self.callbacks);
            let slot: Rc<RefCell<Option<Retained<NSButton>>>> = Rc::default();
            let reader = Rc::clone(&slot);
            (
                ActionTarget::new(mtm, move || {
                    if let Some(button) = reader.borrow().as_ref() {
                        (callbacks.on_night)(button.state() == NSControlStateValueOn);
                    }
                }),
                slot,
            )
        };
        let night = checkbox(
            mtm,
            &texts.night,
            NSRect::new(NSPoint::new(380.0, row3 + 2.0), NSSize::new(110.0, 22.0)),
            &night_target.0,
        );
        *night_target.1.borrow_mut() = Some(night.clone());
        targets.push(night_target.0);

        let row4 = row(TOP_ROWS[3]);
        let width_label = label(
            mtm,
            &texts.width,
            NSRect::new(NSPoint::new(MARGIN, row4 + 4.0), NSSize::new(150.0, 16.0)),
            0.8,
        );
        let (width_slider, width_target) = self.slider_with(
            NSRect::new(NSPoint::new(170.0, row4), NSSize::new(200.0, TOP_ROWS[3])),
            |c, v| (c.on_width)(v),
        );
        targets.push(width_target);

        let mut three_d = |index: usize, text: &str, report: fn(&MixerCallbacks, f64)| {
            let top = row(TOP_ROWS[index]);
            let name = label(
                mtm,
                text,
                NSRect::new(NSPoint::new(MARGIN, top + 4.0), NSSize::new(150.0, 16.0)),
                0.8,
            );
            let (control, target) = self.slider_with(
                NSRect::new(
                    NSPoint::new(170.0, top),
                    NSSize::new(200.0, TOP_ROWS[index]),
                ),
                report,
            );
            targets.push(target);
            self.root.addSubview(&name);
            self.root.addSubview(&control);
            control
        };
        let spatial = three_d(4, &texts.spatial, |c, v| (c.on_spatial)(v));
        let angle = three_d(5, &texts.angle, |c, v| (c.on_angle)(v));
        let orbit = three_d(6, &texts.orbit, |c, v| (c.on_orbit)(v));

        self.root.addSubview(&enabled);
        self.root.addSubview(&device);
        self.root.addSubview(&volume_label);
        self.root.addSubview(&volume);
        self.root.addSubview(&volume_percent);
        self.root.addSubview(&eq);
        self.root.addSubview(&boost_label);
        self.root.addSubview(&boost);
        self.root.addSubview(&night);
        self.root.addSubview(&width_label);
        self.root.addSubview(&width_slider);
        Header {
            enabled,
            device,
            volume_label,
            volume,
            volume_percent,
            eq,
            boost,
            night,
            width: width_slider,
            spatial,
            angle,
            orbit,
            _targets: targets,
        }
    }

    /// A slider whose every move reports its position through `report`.
    fn slider_with(
        &self,
        frame: NSRect,
        report: fn(&MixerCallbacks, f64),
    ) -> (Retained<NSSlider>, Retained<ActionTarget>) {
        let callbacks = Rc::clone(&self.callbacks);
        let slot: Rc<RefCell<Option<Retained<NSSlider>>>> = Rc::default();
        let reader = Rc::clone(&slot);
        let target = ActionTarget::new(self.mtm, move || {
            if let Some(slider) = reader.borrow().as_ref() {
                report(&callbacks, slider.doubleValue());
            }
        });
        let made = slider(self.mtm, frame, &target);
        *slot.borrow_mut() = Some(made.clone());
        (made, target)
    }

    fn icon(&self, path: &str) -> Retained<NSImage> {
        if let Some(icon) = self.icons.borrow().get(path) {
            return icon.clone();
        }
        let icon = NSWorkspace::sharedWorkspace().iconForFile(&NSString::from_str(path));
        self.icons
            .borrow_mut()
            .insert(path.to_owned(), icon.clone());
        icon
    }

    fn build_row(
        &self,
        row: &MixerRow,
        outputs: &[String],
        eq_options: &[String],
        top: f64,
    ) -> RowViews {
        let mtm = self.mtm;
        let width = self.list.frame().size.width;
        let view = NSView::initWithFrame(
            mtm.alloc(),
            NSRect::new(NSPoint::new(0.0, top), NSSize::new(width, ROW_HEIGHT)),
        );
        let middle = (ROW_HEIGHT - ICON) / 2.0;
        let mut targets = Vec::new();

        let icon_view = NSImageView::initWithFrame(
            mtm.alloc(),
            NSRect::new(NSPoint::new(12.0, middle), NSSize::new(ICON, ICON)),
        );
        if let Some(path) = &row.icon_path {
            icon_view.setImage(Some(&self.icon(path)));
        }
        view.addSubview(&icon_view);

        let name = label(
            mtm,
            &row.name,
            NSRect::new(NSPoint::new(38.0, 9.0), NSSize::new(110.0, 16.0)),
            if row.playing { 0.95 } else { 0.55 },
        );
        let menu = NSMenu::new(mtm);
        let title = NSMenuItem::new(mtm);
        title.setTitle(&NSString::from_str(&self.texts.app_eq));
        title.setEnabled(false);
        menu.addItem(&title);
        for (index, option) in eq_options.iter().enumerate() {
            let callbacks = Rc::clone(&self.callbacks);
            let key = row.key.clone();
            let target = ActionTarget::new(mtm, move || (callbacks.on_eq)(&key, index));
            // SAFETY: `fire:` takes the sender as its only argument; the target lives with the row.
            let item = unsafe {
                NSMenuItem::initWithTitle_action_keyEquivalent(
                    NSMenuItem::alloc(mtm),
                    &NSString::from_str(option),
                    Some(sel!(fire:)),
                    &NSString::from_str(""),
                )
            };
            // SAFETY: as above.
            unsafe { item.setTarget(Some(&target)) };
            item.setState(on_off(index == row.eq));
            menu.addItem(&item);
            targets.push(target);
        }
        // SAFETY: a plain NSMenu, kept alive by the row as well.
        unsafe { name.setMenu(Some(&menu)) };
        view.addSubview(&name);

        let (slider_view, slider_target) = {
            let callbacks = Rc::clone(&self.callbacks);
            let key = row.key.clone();
            let slot: Rc<RefCell<Option<Retained<NSSlider>>>> = Rc::default();
            let reader = Rc::clone(&slot);
            let target = ActionTarget::new(mtm, move || {
                if let Some(slider) = reader.borrow().as_ref() {
                    (callbacks.on_volume)(&key, slider.doubleValue());
                }
            });
            let made = slider(
                mtm,
                NSRect::new(NSPoint::new(152.0, 6.0), NSSize::new(140.0, 22.0)),
                &target,
            );
            *slot.borrow_mut() = Some(made.clone());
            (made, target)
        };
        view.addSubview(&slider_view);
        targets.push(slider_target);

        let percent = label(
            mtm,
            &row.percent,
            NSRect::new(NSPoint::new(296.0, 9.0), NSSize::new(42.0, 16.0)),
            0.8,
        );
        percent.setAlignment(NSTextAlignment::Right);
        view.addSubview(&percent);

        let (mute, mute_target) = {
            let callbacks = Rc::clone(&self.callbacks);
            let key = row.key.clone();
            let slot: Rc<RefCell<Option<Retained<NSButton>>>> = Rc::default();
            let reader = Rc::clone(&slot);
            let target = ActionTarget::new(mtm, move || {
                if let Some(button) = reader.borrow().as_ref() {
                    (callbacks.on_mute)(&key, button.state() == NSControlStateValueOn);
                }
            });
            let made = mute_button(
                mtm,
                &self.texts.mute,
                NSRect::new(NSPoint::new(360.0, 5.0), NSSize::new(24.0, 24.0)),
                &target,
            );
            *slot.borrow_mut() = Some(made.clone());
            (made, target)
        };
        view.addSubview(&mute);
        targets.push(mute_target);

        let (output, output_target) = {
            let callbacks = Rc::clone(&self.callbacks);
            let key = row.key.clone();
            let slot: Rc<RefCell<Option<Retained<NSPopUpButton>>>> = Rc::default();
            let reader = Rc::clone(&slot);
            let target = ActionTarget::new(mtm, move || {
                if let Some(popup) = reader.borrow().as_ref() {
                    (callbacks.on_output)(&key, selected(popup));
                }
            });
            let made = popup(
                mtm,
                NSRect::new(NSPoint::new(400.0, 5.0), NSSize::new(92.0, 24.0)),
                outputs,
                &target,
            );
            *slot.borrow_mut() = Some(made.clone());
            (made, target)
        };
        view.addSubview(&output);
        targets.push(output_target);

        self.list.addSubview(&view);
        let views = RowViews {
            key: row.key.clone(),
            view,
            name,
            slider: slider_view,
            percent,
            mute,
            output,
            menu,
            _targets: targets,
        };
        Self::fill_row(&views, row, outputs, eq_options);
        views
    }

    fn fill_row(views: &RowViews, row: &MixerRow, outputs: &[String], eq_options: &[String]) {
        views.name.setStringValue(&NSString::from_str(&row.name));
        views.name.setTextColor(Some(&NSColor::colorWithWhite_alpha(
            1.0,
            if row.playing { 0.95 } else { 0.55 },
        )));
        views.slider.setDoubleValue(row.volume);
        views.slider.setEnabled(!row.muted);
        views
            .percent
            .setStringValue(&NSString::from_str(&row.percent));
        views.mute.setState(on_off(row.muted));
        let ink = if row.muted {
            Tone::Red.color()
        } else {
            style::white(style::TEXT_SECONDARY)
        };
        views.mute.setContentTintColor(Some(&ink));
        set_items(&views.output, outputs);
        views.output.selectItemAtIndex(row.output as isize);
        for (index, _) in eq_options.iter().enumerate() {
            // The first menu item is the title.
            if let Some(item) = views.menu.itemAtIndex(index as isize + 1) {
                item.setState(on_off(index == row.eq));
            }
        }
    }

    /// Shows a new state. Rows are rebuilt only when the set of apps or their order changed.
    pub fn show(&self, model: &MixerModel) {
        if *self.model.borrow() == *model {
            return;
        }
        if let Some(h) = self.header.borrow().as_ref() {
            h.enabled.setState(on_off(model.enabled));
            h.device
                .setStringValue(&NSString::from_str(&model.device_title));
            let software = model.device_volume.is_some();
            h.volume_label.setHidden(!software);
            h.volume_percent.setHidden(!software);
            h.volume.setHidden(!software);
            if let Some((position, percent)) = &model.device_volume {
                h.volume.setDoubleValue(*position);
                h.volume_percent
                    .setStringValue(&NSString::from_str(percent));
            }
            set_items(&h.eq, &model.eq_options);
            h.eq.selectItemAtIndex(model.effects_eq as isize);
            h.boost.setDoubleValue(model.boost);
            h.night.setState(on_off(model.night));
            h.width.setDoubleValue(model.width);
            h.spatial.setDoubleValue(model.spatial);
            h.angle.setDoubleValue(model.angle);
            h.orbit.setDoubleValue(model.orbit);
        }

        let same_rows = {
            let rows = self.rows.borrow();
            rows.len() == model.rows.len()
                && rows.iter().zip(&model.rows).all(|(v, r)| v.key == r.key)
        };
        if same_rows {
            for (views, row) in self.rows.borrow().iter().zip(&model.rows) {
                Self::fill_row(views, row, &model.outputs, &model.app_eq_options);
            }
        } else {
            for old in self.rows.borrow_mut().drain(..) {
                old.view.removeFromSuperview();
            }
            let visible = self.scroll.frame().size.height;
            let height = (model.rows.len() as f64 * ROW_HEIGHT).max(visible);
            let width = self.list.frame().size.width;
            self.list.setFrameSize(NSSize::new(width, height));
            let built: Vec<RowViews> = model
                .rows
                .iter()
                .enumerate()
                .map(|(i, row)| {
                    self.build_row(
                        row,
                        &model.outputs,
                        &model.app_eq_options,
                        i as f64 * ROW_HEIGHT,
                    )
                })
                .collect();
            *self.rows.borrow_mut() = built;
        }
        let notice = model.notice.clone().unwrap_or_default();
        self.notice.setStringValue(&NSString::from_str(&notice));
        self.notice.setHidden(notice.is_empty());
        *self.model.borrow_mut() = model.clone();
    }

    /// The notch tab for this view.
    pub fn page(
        self: &Rc<Self>,
        on_show: impl Fn() + 'static,
        on_hide: impl Fn() + 'static,
    ) -> Page {
        Page {
            id: "sound",
            title: self.texts.tab.clone(),
            symbol: "speaker.wave.2",
            root: Retained::into_super(Retained::clone(&self.root)),
            on_show: Box::new(on_show),
            on_hide: Box::new(on_hide),
            on_key: Box::new(|_: Key| false),
            accepts_files: false,
            size: None,
        }
    }
}
