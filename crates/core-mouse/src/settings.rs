use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub smooth: bool,
    /// Reverses mouse wheels only; the trackpad keeps the system direction.
    pub reverse: bool,
    /// Pixels per wheel line.
    pub step_px: f64,
    pub speed: f64,
    pub duration_ms: f64,
    pub fps: f64,
    /// Hold Shift and turn the wheel to scroll sideways, smoothly.
    pub shift_horizontal: bool,
    /// Hold Option and turn the wheel to scroll `boost` times as far.
    pub option_boost: bool,
    pub boost: f64,
    /// Hold Command and turn the wheel to zoom (sends Command+Plus and Command+Minus).
    pub command_zoom: bool,
    /// Per-app overrides keyed by bundle identifier.
    ///
    /// With the global switch on, an app set to off is a block list entry; with it off, an app set to on
    /// is an allow list entry.
    pub apps: HashMap<String, AppRule>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppRule {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub smooth: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reverse: Option<bool>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            smooth: true,
            reverse: false,
            step_px: 36.0,
            speed: 1.0,
            duration_ms: 360.0,
            fps: 120.0,
            shift_horizontal: true,
            option_boost: true,
            boost: 5.0,
            command_zoom: false,
            apps: HashMap::new(),
        }
    }
}

impl Settings {
    /// `(smooth, reverse)` after applying the override for `bundle_id`, if any.
    pub fn effective(&self, bundle_id: &str) -> (bool, bool) {
        let rule = self.apps.get(bundle_id);
        (
            rule.and_then(|r| r.smooth).unwrap_or(self.smooth),
            rule.and_then(|r| r.reverse).unwrap_or(self.reverse),
        )
    }

    /// True when this app has been switched off explicitly. Such an app is left entirely alone: no
    /// smoothing, no sideways scrolling, no boost and no zoom.
    pub fn is_blocked(&self, bundle_id: &str) -> bool {
        self.apps.get(bundle_id).and_then(|r| r.smooth) == Some(false)
    }

    /// Flips smoothing for one app, writing an explicit override that survives later changes to the global switch.
    pub fn toggle_smooth_for(&mut self, bundle_id: &str) {
        let now = self.effective(bundle_id).0;
        self.apps.entry(bundle_id.to_owned()).or_default().smooth = Some(!now);
    }

    /// Clamps hand-edited values into ranges the engine can run with.
    pub fn sanitized(mut self) -> Self {
        self.step_px = self.step_px.clamp(1.0, 400.0);
        self.speed = self.speed.clamp(0.1, 10.0);
        self.duration_ms = self.duration_ms.clamp(50.0, 2000.0);
        self.fps = self.fps.clamp(30.0, 240.0);
        self.boost = self.boost.clamp(1.5, 20.0);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn per_app_rules_override_globals_field_by_field() {
        let mut s = Settings {
            reverse: true,
            ..Settings::default()
        };
        s.apps.insert(
            "com.game".into(),
            AppRule {
                smooth: Some(false),
                reverse: None,
            },
        );
        assert_eq!(s.effective("com.other"), (true, true));
        assert_eq!(s.effective("com.game"), (false, true));
    }

    #[test]
    fn toggling_one_app_leaves_the_others_alone() {
        let mut s = Settings::default();
        s.toggle_smooth_for("com.game");
        assert_eq!(s.effective("com.game"), (false, false));
        assert_eq!(s.effective("com.other"), (true, false));
        s.toggle_smooth_for("com.game");
        assert_eq!(s.effective("com.game"), (true, false));
        // The override is explicit: turning the global switch off no longer affects this app.
        s.smooth = false;
        assert_eq!(s.effective("com.game"), (true, false));
    }

    #[test]
    fn unset_overrides_are_not_written_out() {
        let mut s = Settings::default();
        s.toggle_smooth_for("com.game");
        let json = serde_json::to_string(&s).unwrap();
        assert!(json.contains(r#""com.game":{"smooth":false}"#), "{json}");
    }

    #[test]
    fn partial_json_fills_in_defaults() {
        let s: Settings =
            serde_json::from_str(r#"{"reverse": true, "apps": {"a.b": {"smooth": false}}}"#)
                .unwrap();
        assert!(s.reverse && s.smooth);
        assert_eq!(s.step_px, Settings::default().step_px);
        assert_eq!(s.effective("a.b"), (false, true));
    }

    #[test]
    fn modifier_behaviors_default_to_the_safe_ones() {
        let s = Settings::default();
        assert!(s.shift_horizontal && s.option_boost && !s.command_zoom);
        assert_eq!(s.boost, 5.0);
        // Older config files do not have these keys; they must load with the defaults.
        let old: Settings = serde_json::from_str(r#"{"smooth": false}"#).unwrap();
        assert!(old.shift_horizontal && !old.command_zoom);
    }

    #[test]
    fn sanitizing_clamps_absurd_values() {
        let s = Settings {
            step_px: 0.0,
            speed: 99.0,
            duration_ms: -5.0,
            fps: 1.0,
            ..Settings::default()
        }
        .sanitized();
        assert_eq!(
            (s.step_px, s.speed, s.duration_ms, s.fps),
            (1.0, 10.0, 50.0, 30.0)
        );
        let s = Settings {
            boost: 500.0,
            ..Settings::default()
        }
        .sanitized();
        assert_eq!(s.boost, 20.0);
    }
}
