//! The agents page: the command that waits for an answer, the sessions that are running, and what is left
//! of the usage limits. Everything on it is plain data from the app, drawn in the notch's style.

use crate::{
    button::{Look, TouchButton},
    labels,
    notch::Page,
    panel::Key,
    style::{self, Tone, Weight, white},
    widgets::{Fit, PageRoot, draw_text, text_attributes},
};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained};
use objc2_app_kit::{NSFont, NSStringDrawing, NSView};
use objc2_foundation::{NSObjectProtocol, NSPoint, NSRect, NSSize, NSString};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

const MARGIN: f64 = 18.0;
const GAP: f64 = 10.0;
const ROW_HEIGHT: f64 = 38.0;
const HEADING_HEIGHT: f64 = 22.0;
const USAGE_ROW: f64 = 26.0;
const TOKENS_ROW: f64 = 22.0;
const BUTTON_HEIGHT: f64 = 32.0;
/// The round button at the end of the usage heading.
const REFRESH_SIZE: f64 = 24.0;
/// A command is shown over this many lines; the card is as tall as it needs for them.
const COMMAND_LINES: usize = 3;
const COMMAND_LINE_HEIGHT: f64 = 16.0;
const CARD_PAD: f64 = 16.0;

/// A command or edit that waits for an answer.
#[derive(Debug, Clone)]
pub struct ApprovalCardData {
    pub id: u64,
    /// "Claude Code wants to use Bash", drawn in capitals.
    pub heading: String,
    pub command: String,
    /// The folder and how long ago it asked.
    pub detail: String,
    pub allow: String,
    pub deny: String,
}

#[derive(Debug, Clone)]
pub struct SessionData {
    pub tone: Tone,
    pub agent: String,
    pub project: String,
    pub state: String,
    pub age: String,
}

#[derive(Debug, Clone)]
pub struct UsageData {
    pub label: String,
    /// How much is used, 0 to 1; `None` when it is not known.
    pub fraction: Option<f64>,
    pub value: String,
    pub reset: String,
}

/// One line of the tokens section: what an agent used today and this week, and its cost at API list prices.
#[derive(Debug, Clone)]
pub struct TokensData {
    pub label: String,
    /// "1.2M tokens  $3.45" for today.
    pub today: String,
    /// "Week $21.30".
    pub week: String,
}

pub struct AgentsDisplay {
    /// The first command in line; the others wait behind it.
    pub approval: Option<ApprovalCardData>,
    /// "2 more waiting", or empty.
    pub more_waiting: String,
    pub sessions_heading: String,
    pub sessions: Vec<SessionData>,
    pub usage_heading: String,
    pub usage: Vec<UsageData>,
    /// The name of the button that reads the usage limits again, which is also its tooltip; `None` leaves it out.
    pub refresh_label: Option<String>,
    /// A refresh is under way, so the button is off until it ends.
    pub refreshing: bool,
    pub tokens_heading: String,
    pub tokens: Vec<TokensData>,
    pub empty_title: String,
    pub empty_hint: String,
}

pub struct AgentsCallbacks {
    /// An answer for the command with this id: true to allow it.
    pub on_decide: Box<dyn Fn(u64, bool)>,
    /// The usage limits are to be read again.
    pub on_refresh: Box<dyn Fn()>,
}

fn attrs(
    size: f64,
    weight: Weight,
    alpha: f64,
    fit: Fit,
) -> Retained<objc2_foundation::NSMutableDictionary<NSString, objc2::runtime::AnyObject>> {
    text_attributes(&style::font(size, weight), &white(alpha), fit, false)
}

fn text_width(text: &str, font: &NSFont) -> f64 {
    let attrs = text_attributes(font, &white(1.0), Fit::Clip, false);
    // SAFETY: the attributes dictionary holds valid attribute objects.
    unsafe { NSString::from_str(text).sizeWithAttributes(Some(&attrs)) }
        .width
        .ceil()
}

/// Cuts a command to what fits over `lines` lines of the given width, ending in "..." when it is cut. Meant
/// for a fixed-width font, where the count of characters per line is exact.
pub fn fit_command(command: &str, per_line: usize, lines: usize) -> String {
    let room = per_line.max(4) * lines.max(1);
    let flat: String = command
        .chars()
        .map(|c| if c == '\n' { ' ' } else { c })
        .collect();
    if flat.chars().count() <= room {
        return flat;
    }
    let mut cut: String = flat.chars().take(room - 3).collect();
    cut.push_str("...");
    cut
}

pub struct CardIvars {
    data: RefCell<Option<ApprovalCardData>>,
}

define_class!(
    // SAFETY: NSView allows subclassing; the ivars are only touched on the main thread and need no Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigApprovalCard"]
    #[ivars = CardIvars]
    struct ApprovalCard;

    impl ApprovalCard {
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }

        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            let bounds = self.bounds();
            style::card(bounds, style::CARD_RADIUS);
            let mut edge = Tone::Orange.color();
            edge = edge.colorWithAlphaComponent(0.5);
            style::stroke_rounded(bounds, style::CARD_RADIUS, &edge, 1.2);
            let data = self.ivars().data.borrow();
            let Some(data) = data.as_ref() else {
                return;
            };
            let width = bounds.size.width - 2.0 * CARD_PAD;
            draw_text(
                &data.heading.to_uppercase(),
                NSRect::new(NSPoint::new(CARD_PAD, 14.0), NSSize::new(width, 14.0)),
                &text_attributes(
                    &style::font(10.5, Weight::Semibold),
                    &Tone::Orange.color(),
                    Fit::Tail,
                    false,
                ),
            );
            let mono = NSFont::userFixedPitchFontOfSize(12.5)
                .unwrap_or_else(|| style::font(12.5, Weight::Regular));
            let per_line = (width / text_width("M", &mono).max(1.0)).floor() as usize;
            draw_text(
                &fit_command(&data.command, per_line, COMMAND_LINES),
                NSRect::new(
                    NSPoint::new(CARD_PAD, 34.0),
                    NSSize::new(width, COMMAND_LINE_HEIGHT * COMMAND_LINES as f64 + 2.0),
                ),
                &text_attributes(&mono, &white(0.96), Fit::Wrap, false),
            );
            draw_text(
                &data.detail,
                NSRect::new(
                    NSPoint::new(CARD_PAD, 34.0 + COMMAND_LINE_HEIGHT * COMMAND_LINES as f64 + 8.0),
                    NSSize::new(width, 14.0),
                ),
                &attrs(11.0, Weight::Regular, style::TEXT_SECONDARY, Fit::Tail),
            );
        }
    }

    unsafe impl NSObjectProtocol for ApprovalCard {}
);

/// The height a card needs: heading, the command lines, the detail line and the buttons.
const CARD_HEIGHT: f64 = 34.0
    + COMMAND_LINE_HEIGHT * COMMAND_LINES as f64
    + 8.0
    + 14.0
    + 12.0
    + BUTTON_HEIGHT
    + CARD_PAD;

pub struct ListIvars {
    rows: RefCell<Vec<SessionData>>,
}

define_class!(
    // SAFETY: NSView allows subclassing; the ivars are only touched on the main thread and need no Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigAgentSessions"]
    #[ivars = ListIvars]
    struct SessionList;

    impl SessionList {
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }

        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            let width = self.bounds().size.width;
            let name_font = style::font(13.0, Weight::Semibold);
            for (i, row) in self.ivars().rows.borrow().iter().enumerate() {
                let top = i as f64 * ROW_HEIGHT;
                let card = NSRect::new(
                    NSPoint::new(0.0, top + 2.0),
                    NSSize::new(width, ROW_HEIGHT - 4.0),
                );
                style::card(card, 12.0);
                let middle = top + ROW_HEIGHT / 2.0;
                style::fill_circle(
                    NSRect::new(NSPoint::new(14.0, middle - 4.0), NSSize::new(8.0, 8.0)),
                    &row.tone.color(),
                );
                let mut x = 32.0;
                let agent_width = text_width(&row.agent, &name_font);
                draw_text(
                    &row.agent,
                    NSRect::new(NSPoint::new(x, middle - 9.0), NSSize::new(agent_width + 2.0, 18.0)),
                    &text_attributes(&name_font, &white(style::TEXT), Fit::Clip, false),
                );
                x += agent_width + 10.0;
                let right = width - 14.0;
                let age_width = text_width(&row.age, &style::font(11.0, Weight::Regular)).max(34.0);
                let state_font = style::font(11.0, Weight::Semibold);
                let state_width = text_width(&row.state, &state_font);
                let state_x = right - age_width - 10.0 - state_width;
                draw_text(
                    &row.project,
                    NSRect::new(
                        NSPoint::new(x, middle - 8.0),
                        NSSize::new((state_x - x - 10.0).max(10.0), 16.0),
                    ),
                    &attrs(12.0, Weight::Regular, style::TEXT_SECONDARY, Fit::Tail),
                );
                draw_text(
                    &row.state,
                    NSRect::new(NSPoint::new(state_x, middle - 8.0), NSSize::new(state_width + 2.0, 16.0)),
                    &text_attributes(&state_font, &row.tone.color(), Fit::Clip, false),
                );
                draw_text(
                    &row.age,
                    NSRect::new(NSPoint::new(right - age_width, middle - 8.0), NSSize::new(age_width, 16.0)),
                    &attrs(11.0, Weight::Regular, style::TEXT_TERTIARY + 0.1, Fit::Clip),
                );
            }
        }
    }

    unsafe impl NSObjectProtocol for SessionList {}
);

pub struct UsageIvars {
    rows: RefCell<Vec<UsageData>>,
}

define_class!(
    // SAFETY: NSView allows subclassing; the ivars are only touched on the main thread and need no Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigAgentUsage"]
    #[ivars = UsageIvars]
    struct UsageBars;

    impl UsageBars {
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }

        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            let width = self.bounds().size.width;
            let label_width = 74.0;
            let value_width = 40.0;
            let reset_width = 104.0;
            let bar_x = label_width + 8.0;
            let bar_width = (width - bar_x - 8.0 - value_width - reset_width).max(40.0);
            for (i, row) in self.ivars().rows.borrow().iter().enumerate() {
                let middle = i as f64 * USAGE_ROW + USAGE_ROW / 2.0;
                draw_text(
                    &row.label,
                    NSRect::new(NSPoint::new(0.0, middle - 8.0), NSSize::new(label_width, 16.0)),
                    &attrs(11.5, Weight::Medium, style::TEXT_SECONDARY, Fit::Tail),
                );
                let track = NSRect::new(
                    NSPoint::new(bar_x, middle - 3.0),
                    NSSize::new(bar_width, 6.0),
                );
                style::well(track, 3.0);
                if let Some(fraction) = row.fraction {
                    let fraction = fraction.clamp(0.0, 1.0);
                    let tone = if fraction >= 0.9 {
                        Tone::Red
                    } else if fraction >= 0.7 {
                        Tone::Orange
                    } else {
                        Tone::Blue
                    };
                    let fill = NSRect::new(
                        track.origin,
                        NSSize::new((track.size.width * fraction).max(track.size.height), track.size.height),
                    );
                    style::fill_capsule(fill, &tone.color());
                }
                let right = bar_x + bar_width + 10.0;
                draw_text(
                    &row.value,
                    NSRect::new(NSPoint::new(right, middle - 8.0), NSSize::new(value_width, 16.0)),
                    &attrs(11.5, Weight::Semibold, style::TEXT, Fit::Clip),
                );
                draw_text(
                    &row.reset,
                    NSRect::new(NSPoint::new(right + value_width, middle - 8.0), NSSize::new(reset_width, 16.0)),
                    &attrs(11.0, Weight::Regular, style::TEXT_TERTIARY + 0.1, Fit::Tail),
                );
            }
        }
    }

    unsafe impl NSObjectProtocol for UsageBars {}
);

pub struct TokenIvars {
    rows: RefCell<Vec<TokensData>>,
}

define_class!(
    // SAFETY: NSView allows subclassing; the ivars are only touched on the main thread and need no Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigAgentTokens"]
    #[ivars = TokenIvars]
    struct TokenRows;

    impl TokenRows {
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }

        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            let width = self.bounds().size.width;
            let (label_width, week_width) = (74.0, 120.0);
            for (i, row) in self.ivars().rows.borrow().iter().enumerate() {
                let middle = i as f64 * TOKENS_ROW + TOKENS_ROW / 2.0;
                draw_text(
                    &row.label,
                    NSRect::new(NSPoint::new(0.0, middle - 8.0), NSSize::new(label_width, 16.0)),
                    &attrs(11.5, Weight::Medium, style::TEXT_SECONDARY, Fit::Tail),
                );
                draw_text(
                    &row.today,
                    NSRect::new(
                        NSPoint::new(label_width + 8.0, middle - 8.0),
                        NSSize::new((width - label_width - 8.0 - week_width).max(40.0), 16.0),
                    ),
                    &attrs(11.5, Weight::Semibold, style::TEXT, Fit::Tail),
                );
                draw_text(
                    &row.week,
                    NSRect::new(NSPoint::new(width - week_width, middle - 8.0), NSSize::new(week_width, 16.0)),
                    &text_attributes(
                        &style::font(11.0, Weight::Regular),
                        &white(style::TEXT_TERTIARY + 0.1),
                        Fit::Tail,
                        false,
                    ),
                );
            }
        }
    }

    unsafe impl NSObjectProtocol for TokenRows {}
);

pub struct NoteIvars {
    title: RefCell<String>,
    hint: RefCell<String>,
}

define_class!(
    // SAFETY: NSView allows subclassing; the ivars are only touched on the main thread and need no Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigAgentEmpty"]
    #[ivars = NoteIvars]
    struct EmptyNote;

    impl EmptyNote {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            let bounds = self.bounds();
            let middle = bounds.size.height / 2.0;
            style::draw_symbol("sparkles", NSPoint::new(bounds.size.width / 2.0, middle + 44.0), 34.0, &white(0.34));
            draw_text(
                &self.ivars().title.borrow(),
                NSRect::new(NSPoint::new(20.0, middle - 4.0), NSSize::new(bounds.size.width - 40.0, 20.0)),
                &text_attributes(&style::font(14.0, Weight::Semibold), &white(0.8), Fit::Clip, true),
            );
            draw_text(
                &self.ivars().hint.borrow(),
                NSRect::new(NSPoint::new(40.0, middle - 56.0), NSSize::new(bounds.size.width - 80.0, 48.0)),
                &text_attributes(&style::font(12.0, Weight::Regular), &white(style::TEXT_SECONDARY), Fit::Wrap, true),
            );
        }
    }

    unsafe impl NSObjectProtocol for EmptyNote {}
);

fn new_view<T>(alloc_and_init: impl FnOnce() -> Retained<T>) -> Retained<T> {
    alloc_and_init()
}

pub struct AgentsView {
    pub root: Retained<PageRoot>,
    frame: NSRect,
    dynamic: RefCell<Vec<Retained<NSView>>>,
    first: Cell<Option<u64>>,
    callbacks: Rc<AgentsCallbacks>,
}

impl AgentsView {
    pub fn new(mtm: MainThreadMarker, frame: NSRect, callbacks: AgentsCallbacks) -> Rc<Self> {
        let root = PageRoot::new(mtm, frame, &labels::get().agents_hint);
        Rc::new(Self {
            root,
            frame,
            dynamic: RefCell::new(Vec::new()),
            first: Cell::new(None),
            callbacks: Rc::new(callbacks),
        })
    }

    fn heading(&self, mtm: MainThreadMarker, text: &str, y_from_top: f64) -> Retained<NSView> {
        let note = new_view(|| {
            let this = mtm.alloc::<HeadingLabel>().set_ivars(HeadingIvars {
                text: RefCell::new(text.to_uppercase()),
            });
            // SAFETY: designated initializer with valid arguments on the main thread.
            unsafe {
                msg_send![super(this), initWithFrame: NSRect::new(
                    NSPoint::new(MARGIN + 2.0, self.frame.size.height - y_from_top - HEADING_HEIGHT),
                    NSSize::new(self.frame.size.width - 2.0 * MARGIN, HEADING_HEIGHT),
                )]
            }
        });
        Retained::into_super(note)
    }

    /// Replaces what the page shows.
    pub fn show(&self, mtm: MainThreadMarker, display: &AgentsDisplay) {
        for view in self.dynamic.borrow_mut().drain(..) {
            view.removeFromSuperview();
        }
        let mut added: Vec<Retained<NSView>> = Vec::new();
        let (width, top) = (self.frame.size.width, self.frame.size.height);
        let inner = width - 2.0 * MARGIN;
        let mut cursor = 0.0;
        self.first.set(display.approval.as_ref().map(|a| a.id));

        if let Some(approval) = &display.approval {
            let card = {
                let this = mtm.alloc::<ApprovalCard>().set_ivars(CardIvars {
                    data: RefCell::new(Some(approval.clone())),
                });
                let frame = NSRect::new(
                    NSPoint::new(MARGIN, top - cursor - CARD_HEIGHT),
                    NSSize::new(inner, CARD_HEIGHT),
                );
                // SAFETY: designated initializer with valid arguments on the main thread.
                let card: Retained<ApprovalCard> =
                    unsafe { msg_send![super(this), initWithFrame: frame] };
                card
            };
            let buttons_top = CARD_HEIGHT - CARD_PAD - BUTTON_HEIGHT;
            let half = (inner - 2.0 * CARD_PAD - GAP) / 2.0;
            let deny = TouchButton::new(
                mtm,
                NSRect::new(
                    NSPoint::new(CARD_PAD, buttons_top),
                    NSSize::new(half, BUTTON_HEIGHT),
                ),
                Look::Rect,
            );
            deny.set_symbol("xmark");
            deny.set_title(&approval.deny);
            deny.set_tone(Some(Tone::Red));
            let allow = TouchButton::new(
                mtm,
                NSRect::new(
                    NSPoint::new(CARD_PAD + half + GAP, buttons_top),
                    NSSize::new(half, BUTTON_HEIGHT),
                ),
                Look::Rect,
            );
            allow.set_symbol("checkmark");
            allow.set_title(&approval.allow);
            allow.set_selected(true);
            let id = approval.id;
            let shared = Rc::clone(&self.callbacks);
            deny.set_on_click(move || (shared.on_decide)(id, false));
            let shared = Rc::clone(&self.callbacks);
            allow.set_on_click(move || (shared.on_decide)(id, true));
            card.addSubview(&deny);
            card.addSubview(&allow);
            self.root.addSubview(&card);
            added.push(Retained::into_super(card));
            cursor += CARD_HEIGHT + 8.0;
            if !display.more_waiting.is_empty() {
                let more = new_view(|| {
                    let this = mtm.alloc::<HeadingLabel>().set_ivars(HeadingIvars {
                        text: RefCell::new(display.more_waiting.clone()),
                    });
                    // SAFETY: designated initializer with valid arguments on the main thread.
                    unsafe {
                        msg_send![super(this), initWithFrame: NSRect::new(
                            NSPoint::new(MARGIN + 2.0, top - cursor - 16.0),
                            NSSize::new(inner, 16.0),
                        )]
                    }
                });
                let more: Retained<NSView> = Retained::into_super(more);
                self.root.addSubview(&more);
                added.push(more);
                cursor += 20.0;
            }
        }

        // The section is there for its button too: with nothing read yet, a refresh is how the first numbers come.
        let usage_shown = !display.usage.is_empty() || display.refresh_label.is_some();
        let usage_height = if usage_shown {
            HEADING_HEIGHT + display.usage.len() as f64 * USAGE_ROW + 8.0
        } else {
            0.0
        } + if display.tokens.is_empty() {
            0.0
        } else {
            HEADING_HEIGHT + display.tokens.len() as f64 * TOKENS_ROW + 8.0
        };
        if !display.sessions.is_empty() {
            let room = top - cursor - usage_height - 28.0;
            let rows = (((room - HEADING_HEIGHT) / ROW_HEIGHT).floor().max(0.0) as usize)
                .min(display.sessions.len());
            if rows > 0 {
                let heading = self.heading(mtm, &display.sessions_heading, cursor);
                self.root.addSubview(&heading);
                added.push(heading);
                cursor += HEADING_HEIGHT;
                let list = {
                    let this = mtm.alloc::<SessionList>().set_ivars(ListIvars {
                        rows: RefCell::new(display.sessions[..rows].to_vec()),
                    });
                    let frame = NSRect::new(
                        NSPoint::new(MARGIN, top - cursor - rows as f64 * ROW_HEIGHT),
                        NSSize::new(inner, rows as f64 * ROW_HEIGHT),
                    );
                    // SAFETY: designated initializer with valid arguments on the main thread.
                    let list: Retained<SessionList> =
                        unsafe { msg_send![super(this), initWithFrame: frame] };
                    list
                };
                self.root.addSubview(&list);
                added.push(Retained::into_super(list));
                cursor += rows as f64 * ROW_HEIGHT + 8.0;
            }
        }
        if usage_shown {
            let heading = self.heading(mtm, &display.usage_heading, cursor);
            self.root.addSubview(&heading);
            added.push(heading);
            if let Some(label) = &display.refresh_label {
                let button = TouchButton::new(
                    mtm,
                    NSRect::new(
                        NSPoint::new(
                            width - MARGIN - 2.0 - REFRESH_SIZE,
                            top - cursor - HEADING_HEIGHT + (HEADING_HEIGHT - REFRESH_SIZE) / 2.0,
                        ),
                        NSSize::new(REFRESH_SIZE, REFRESH_SIZE),
                    ),
                    Look::Round { filled: false },
                );
                button.set_symbol("arrow.clockwise");
                button.set_title(label);
                button.set_enabled(!display.refreshing);
                let shared = Rc::clone(&self.callbacks);
                button.set_on_click(move || (shared.on_refresh)());
                self.root.addSubview(&button);
                added.push(Retained::into_super(button));
            }
            cursor += HEADING_HEIGHT;
            if !display.usage.is_empty() {
                let bars = {
                    let this = mtm.alloc::<UsageBars>().set_ivars(UsageIvars {
                        rows: RefCell::new(display.usage.clone()),
                    });
                    let height = display.usage.len() as f64 * USAGE_ROW;
                    let frame = NSRect::new(
                        NSPoint::new(MARGIN + 2.0, top - cursor - height),
                        NSSize::new(inner - 4.0, height),
                    );
                    // SAFETY: designated initializer with valid arguments on the main thread.
                    let bars: Retained<UsageBars> =
                        unsafe { msg_send![super(this), initWithFrame: frame] };
                    bars
                };
                self.root.addSubview(&bars);
                added.push(Retained::into_super(bars));
            }
            cursor += display.usage.len() as f64 * USAGE_ROW + 8.0;
        }
        if !display.tokens.is_empty() {
            let heading = self.heading(mtm, &display.tokens_heading, cursor);
            self.root.addSubview(&heading);
            added.push(heading);
            cursor += HEADING_HEIGHT;
            let rows = {
                let this = mtm.alloc::<TokenRows>().set_ivars(TokenIvars {
                    rows: RefCell::new(display.tokens.clone()),
                });
                let height = display.tokens.len() as f64 * TOKENS_ROW;
                let frame = NSRect::new(
                    NSPoint::new(MARGIN + 2.0, top - cursor - height),
                    NSSize::new(inner - 4.0, height),
                );
                // SAFETY: designated initializer with valid arguments on the main thread.
                let rows: Retained<TokenRows> =
                    unsafe { msg_send![super(this), initWithFrame: frame] };
                rows
            };
            self.root.addSubview(&rows);
            added.push(Retained::into_super(rows));
        }
        if display.approval.is_none()
            && display.sessions.is_empty()
            && display.usage.is_empty()
            && display.tokens.is_empty()
        {
            let note = {
                let this = mtm.alloc::<EmptyNote>().set_ivars(NoteIvars {
                    title: RefCell::new(display.empty_title.clone()),
                    hint: RefCell::new(display.empty_hint.clone()),
                });
                let frame = NSRect::new(NSPoint::new(0.0, 24.0), NSSize::new(width, top - 24.0));
                // SAFETY: designated initializer with valid arguments on the main thread.
                let note: Retained<EmptyNote> =
                    unsafe { msg_send![super(this), initWithFrame: frame] };
                note
            };
            self.root.addSubview(&note);
            added.push(Retained::into_super(note));
        }
        *self.dynamic.borrow_mut() = added;
    }

    /// Returns true when the key was consumed: Enter allows the command in line, Option+Delete denies it, and
    /// Command+R reads the usage limits again.
    pub fn handle_key(&self, key: Key) -> bool {
        if key == Key::Shortcut('r') {
            (self.callbacks.on_refresh)();
            return true;
        }
        let Some(id) = self.first.get() else {
            return false;
        };
        match key {
            Key::Enter => (self.callbacks.on_decide)(id, true),
            Key::Delete => (self.callbacks.on_decide)(id, false),
            _ => return false,
        }
        true
    }

    /// The notch tab for this view.
    pub fn page(
        self: &Rc<Self>,
        on_show: impl Fn() + 'static,
        on_hide: impl Fn() + 'static,
    ) -> Page {
        let keys = Rc::clone(self);
        Page {
            id: "agents",
            title: labels::get().agents_tab.clone(),
            symbol: "sparkles",
            root: Retained::into_super(Retained::clone(&self.root)),
            on_show: Box::new(on_show),
            on_hide: Box::new(on_hide),
            on_key: Box::new(move |key| keys.handle_key(key)),
            accepts_files: false,
            size: None,
        }
    }
}

pub struct HeadingIvars {
    text: RefCell<String>,
}

define_class!(
    // SAFETY: NSView allows subclassing; the ivars are only touched on the main thread and need no Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigAgentHeading"]
    #[ivars = HeadingIvars]
    struct HeadingLabel;

    impl HeadingLabel {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            let bounds = self.bounds();
            draw_text(
                &self.ivars().text.borrow(),
                NSRect::new(NSPoint::new(0.0, 3.0), NSSize::new(bounds.size.width, bounds.size.height - 4.0)),
                &attrs(10.5, Weight::Semibold, style::TEXT_TERTIARY + 0.04, Fit::Tail),
            );
        }
    }

    unsafe impl NSObjectProtocol for HeadingLabel {}
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_short_command_is_shown_whole() {
        assert_eq!(fit_command("ls -la", 50, 3), "ls -la");
    }

    #[test]
    fn a_long_command_is_cut_to_the_room_and_says_so() {
        let command = "x".repeat(500);
        let shown = fit_command(&command, 50, 3);
        assert_eq!(shown.chars().count(), 150);
        assert!(shown.ends_with("..."));
    }

    #[test]
    fn line_breaks_become_spaces_so_one_command_stays_one_paragraph() {
        assert_eq!(fit_command("echo a\necho b", 50, 3), "echo a echo b");
    }

    #[test]
    fn a_cut_never_splits_a_character() {
        let shown = fit_command(&"đ".repeat(100), 10, 3);
        assert_eq!(shown.chars().count(), 30);
    }
}
