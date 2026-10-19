//! The permissions check popup: displays all system permissions Figghost uses, checks their live status,
//! and lets the user grant or open the corresponding System Settings pane directly.

use crate::i18n;
use macos_sys::{
    MainThreadMarker, app,
    permissions,
    webview::{WebWindow, WindowSpec},
    widgets::{base64, image_png},
};
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    rc::{Rc, Weak},
};

const PAGE: &str = include_str!("../web/permissions.html");
const APP_ICON: &[u8] = include_bytes!("../assets/Figghost.icns");
const ICON_POINTS: f64 = 48.0;
const ICON_SCALE: f64 = 2.0;

const WIDTH: f64 = 480.0;
const HEIGHT: f64 = 620.0;
const MIN_WIDTH: f64 = 420.0;
const MIN_HEIGHT: f64 = 520.0;
const REFRESH_SECONDS: f64 = 1.0;

fn html() -> String {
    let icon = image_png(APP_ICON, ICON_POINTS, ICON_SCALE).map_or_else(String::new, |png| {
        format!("data:image/png;base64,{}", base64(&png))
    });
    PAGE.replace("/*__ICON__*/", &icon)
}

pub struct PermissionsPopup {
    mtm: MainThreadMarker,
    window: RefCell<Option<Rc<WebWindow>>>,
    timer: RefCell<Option<app::Timer>>,
    me: Weak<PermissionsPopup>,
}

impl PermissionsPopup {
    pub fn new(mtm: MainThreadMarker) -> Rc<Self> {
        Rc::new_cyclic(|me| Self {
            mtm,
            window: RefCell::default(),
            timer: RefCell::default(),
            me: me.clone(),
        })
    }

    /// Opens the permissions window or brings it to the front.
    pub fn show(&self) {
        let existing = self.window.borrow().clone();
        if let Some(window) = existing {
            window.show(self.mtm);
            return;
        }

        let (on_message, on_close) = (self.me.clone(), self.me.clone());
        let window = WebWindow::new(
            self.mtm,
            &WindowSpec {
                title: "Figghost Permissions",
                width: WIDTH,
                height: HEIGHT,
                min_width: MIN_WIDTH,
                min_height: MIN_HEIGHT,
            },
            &html(),
            move |text| {
                if let Some(this) = on_message.upgrade() {
                    this.handle_message(&text);
                }
            },
            move || {
                if let Some(this) = on_close.upgrade() {
                    this.closed();
                }
            },
        );

        *self.window.borrow_mut() = Some(Rc::clone(&window));

        let weak = self.me.clone();
        *self.timer.borrow_mut() = Some(app::every(REFRESH_SECONDS, move || {
            if let Some(this) = weak.upgrade() {
                this.push_state();
            }
        }));

        window.show(self.mtm);
    }

    fn closed(&self) {
        self.timer.borrow_mut().take();
        let weak = self.me.clone();
        app::after(0.0, move || {
            if let Some(this) = weak.upgrade() {
                this.window.borrow_mut().take();
            }
        });
    }

    fn handle_message(&self, text: &str) {
        let Ok(msg) = serde_json::from_str::<Value>(text) else {
            return;
        };
        match msg["t"].as_str() {
            Some("ready") | Some("refresh") => self.push_state(),
            Some("close") => {
                if let Some(window) = self.window.borrow().as_ref() {
                    window.close();
                }
            }
            Some("request") => {
                if let Some(id) = msg["id"].as_str() {
                    self.request_permission(id);
                }
            }
            Some("open_pane") => {
                if let Some(id) = msg["id"].as_str() {
                    self.open_pane_for(id);
                }
            }
            _ => {}
        }
    }

    fn request_permission(&self, id: &str) {
        match id {
            "accessibility" => {
                permissions::accessibility_trusted(true);
                permissions::open_pane(permissions::ACCESSIBILITY_PANE);
            }
            "screen_recording" => {
                permissions::screen_recording_request();
                permissions::open_pane(permissions::SCREEN_RECORDING_PANE);
            }
            "microphone" => {
                permissions::microphone_request();
                permissions::open_pane(permissions::MICROPHONE_PANE);
            }
            "camera" => {
                permissions::camera_request();
                permissions::open_pane(permissions::CAMERA_PANE);
            }
            "calendar" => {
                permissions::calendar_request(|_| {});
                permissions::open_pane(permissions::CALENDARS_PANE);
            }
            "reminders" => {
                permissions::reminders_request(|_| {});
                permissions::open_pane(permissions::REMINDERS_PANE);
            }
            "bluetooth" => permissions::open_pane(permissions::BLUETOOTH_PANE),
            "automation" => permissions::open_pane(permissions::AUTOMATION_PANE),
            _ => {}
        }
        self.push_state();
    }

    fn open_pane_for(&self, id: &str) {
        match id {
            "accessibility" => permissions::open_pane(permissions::ACCESSIBILITY_PANE),
            "screen_recording" => permissions::open_pane(permissions::SCREEN_RECORDING_PANE),
            "microphone" => permissions::open_pane(permissions::MICROPHONE_PANE),
            "camera" => permissions::open_pane(permissions::CAMERA_PANE),
            "calendar" => permissions::open_pane(permissions::CALENDARS_PANE),
            "reminders" => permissions::open_pane(permissions::REMINDERS_PANE),
            "bluetooth" => permissions::open_pane(permissions::BLUETOOTH_PANE),
            "automation" => permissions::open_pane(permissions::AUTOMATION_PANE),
            _ => {}
        }
    }

    fn push_state(&self) {
        let Some(window) = self.window.borrow().clone() else {
            return;
        };

        let items = json!([
            {
                "id": "accessibility",
                "name": "Accessibility",
                "name_vi": "Trợ năng",
                "desc": "Smooth mouse scrolling, window snapping, clipboard pasting, volume HUD.",
                "desc_vi": "Cuộn chuột mượt, chia cửa sổ, dán clipboard, chỉ báo âm lượng.",
                "granted": permissions::accessibility_trusted(false),
            },
            {
                "id": "screen_recording",
                "name": "Screen Recording",
                "name_vi": "Ghi màn hình",
                "desc": "Screenshot capture, video recording, OCR text grab and color picker.",
                "desc_vi": "Chụp ảnh màn hình, quay video, trích xuất chữ OCR và chọn màu.",
                "granted": permissions::screen_recording_trusted(),
            },
            {
                "id": "microphone",
                "name": "Microphone",
                "name_vi": "Micrô",
                "desc": "Voice recording during screen captures.",
                "desc_vi": "Ghi âm giọng nói khi quay video màn hình.",
                "granted": permissions::microphone_trusted(),
            },
            {
                "id": "camera",
                "name": "Camera",
                "name_vi": "Máy ảnh",
                "desc": "Camera bubble overlay in screen recordings.",
                "desc_vi": "Khung hình tròn khuôn mặt khi quay màn hình.",
                "granted": permissions::camera_trusted(),
            },
            {
                "id": "calendar",
                "name": "Calendars",
                "name_vi": "Lịch",
                "desc": "Today and tomorrow schedule, meeting alerts with join button.",
                "desc_vi": "Lịch hôm nay, ngày mai và cảnh báo cuộc họp kèm nút tham gia.",
                "granted": permissions::calendar_trusted(),
            },
            {
                "id": "reminders",
                "name": "Reminders",
                "name_vi": "Lời nhắc",
                "desc": "Due reminders and one-click completion on the notch.",
                "desc_vi": "Lời nhắc đến hạn và đánh dấu hoàn thành nhanh trên notch.",
                "granted": permissions::reminders_trusted(),
            },
            {
                "id": "bluetooth",
                "name": "Bluetooth",
                "name_vi": "Bluetooth",
                "desc": "Connected audio headphones, mouse and keyboard in the notch.",
                "desc_vi": "Nhận biết tai nghe, loa, chuột, bàn phím kết nối trên notch.",
                "granted": true,
            },
            {
                "id": "automation",
                "name": "Automation",
                "name_vi": "Tự động hoá",
                "desc": "Track information and playback controls for Music and Spotify.",
                "desc_vi": "Đọc thông tin bài hát và điều khiển phát nhạc cho Music và Spotify.",
                "granted": true,
            }
        ]);

        let lang = i18n::current().code();
        let payload = json!({
            "t": "state",
            "lang": lang,
            "items": items,
        });

        window.host.post(&payload.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_permissions_page_is_self_contained() {
        let content = html();
        assert!(!content.contains("/*__ICON__*/"));
        assert!(content.contains("Content-Security-Policy"));
        for forbidden in ["src=\"http", "href=\"http", "url(http", "@import", "<link "] {
            assert!(!content.contains(forbidden), "found {forbidden}");
        }
    }
}
