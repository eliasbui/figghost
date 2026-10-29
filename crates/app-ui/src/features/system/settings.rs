//! This feature's part of the settings model: the JSON the page renders and the changes it may make.
//! Pure data with no AppKit, so every rule is covered by plain unit tests.

use super::{
    Config, SECONDS,
    logic::{Figure, Kind},
};
use crate::settings::model::Fault;
use serde_json::{Value, json};

/// The state under the key "system". `web/features/system.fixture.json` must have exactly the same keys; a unit
/// test checks it.
pub fn state(config: &Config) -> Value {
    let config = config.clone().sanitized();
    json!({
        "enabled": config.enabled,
        "seconds": config.seconds,
        "processor": config.processor,
        "memory": config.memory,
        "network": config.network,
        "details": config.details,
        "bar_processor": config.bar_processor,
        "bar_memory": config.bar_memory,
        "bar_network": config.bar_network,
        "bar_gpu": config.bar_gpu,
        "bar_heat": config.bar_heat,
        "bar_disk": config.bar_disk,
        "bar_processor_kind": config.bar_processor_kind,
        "bar_memory_kind": config.bar_memory_kind,
        "bar_network_kind": config.bar_network_kind,
        "bar_gpu_kind": config.bar_gpu_kind,
        "bar_heat_kind": config.bar_heat_kind,
        "bar_disk_kind": config.bar_disk_kind,
        "bar_combined": config.bar_combined,
        "bar_color": config.bar_color,
    })
}

/// A change from the page. `path` is relative to the feature. Values must be validated here; the page is not
/// trusted.
pub fn set(config: &mut Config, path: &str, value: &Value) -> Option<Result<(), Fault>> {
    let bad = || Fault::new("invalid_value", path);
    let switch = |slot: &mut bool| value.as_bool().map(|on| *slot = on).ok_or_else(bad);
    // Only a kind the figure can be drawn as: the cores are the processor's alone. A file reads a name it does not
    // know as the default, but the page must send one of the names exactly.
    let kind = |slot: &mut Kind, figure: Figure| {
        serde_json::from_value::<Kind>(value.clone())
            .ok()
            .filter(|k| k.fits(figure) && json!(k) == *value)
            .map(|k| *slot = k)
            .ok_or_else(bad)
    };
    Some(match path {
        "enabled" => switch(&mut config.enabled),
        "processor" => switch(&mut config.processor),
        "memory" => switch(&mut config.memory),
        "network" => switch(&mut config.network),
        "details" => switch(&mut config.details),
        "bar_processor" => switch(&mut config.bar_processor),
        "bar_memory" => switch(&mut config.bar_memory),
        "bar_network" => switch(&mut config.bar_network),
        "bar_gpu" => switch(&mut config.bar_gpu),
        "bar_heat" => switch(&mut config.bar_heat),
        "bar_disk" => switch(&mut config.bar_disk),
        "bar_combined" => switch(&mut config.bar_combined),
        "bar_color" => switch(&mut config.bar_color),
        "bar_processor_kind" => kind(&mut config.bar_processor_kind, Figure::Processor),
        "bar_memory_kind" => kind(&mut config.bar_memory_kind, Figure::Memory),
        "bar_network_kind" => kind(&mut config.bar_network_kind, Figure::Network),
        "bar_gpu_kind" => kind(&mut config.bar_gpu_kind, Figure::Gpu),
        "bar_heat_kind" => kind(&mut config.bar_heat_kind, Figure::Heat),
        "bar_disk_kind" => kind(&mut config.bar_disk_kind, Figure::Disk),
        "seconds" => value
            .as_u64()
            .and_then(|n| u32::try_from(n).ok())
            .filter(|n| SECONDS.contains(n))
            .map(|n| config.seconds = n)
            .ok_or_else(bad),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(value: &Value) -> Vec<String> {
        let mut keys: Vec<String> = value.as_object().unwrap().keys().cloned().collect();
        keys.sort();
        keys
    }

    #[test]
    fn the_fixture_has_exactly_the_keys_of_the_state() {
        let fixture: Value =
            serde_json::from_str(include_str!("../../../web/features/system.fixture.json"))
                .unwrap();
        assert_eq!(keys(&fixture), keys(&state(&Config::default())));
    }

    #[test]
    fn the_state_starts_off_and_holds_a_hostile_interval_to_a_choice() {
        let s = state(&Config::default());
        assert_eq!(s["enabled"], json!(false));
        assert_eq!(s["seconds"], json!(1));
        let hostile = Config {
            seconds: 4_000_000,
            ..Config::default()
        };
        assert_eq!(state(&hostile)["seconds"], json!(1));
    }

    #[test]
    fn the_switches_are_validated() {
        let mut config = Config::default();
        for path in [
            "enabled",
            "processor",
            "memory",
            "network",
            "details",
            "bar_processor",
            "bar_memory",
            "bar_network",
            "bar_gpu",
            "bar_heat",
            "bar_disk",
            "bar_combined",
            "bar_color",
        ] {
            assert!(
                set(&mut config, path, &json!("yes")).unwrap().is_err(),
                "{path}"
            );
            assert!(
                set(&mut config, path, &json!(null)).unwrap().is_err(),
                "{path}"
            );
            assert!(
                set(&mut config, path, &json!(false)).unwrap().is_ok(),
                "{path}"
            );
        }
        assert!(
            !config.enabled
                && !config.processor
                && !config.memory
                && !config.network
                && !config.details
        );
        assert!(
            set(&mut config, "bar_memory", &json!(true))
                .unwrap()
                .is_ok()
                && config.bar_memory
        );
        assert!(set(&mut config, "nonsense", &json!(1)).is_none());
    }

    #[test]
    fn a_figure_takes_only_a_kind_it_can_be_drawn_as() {
        let mut config = Config::default();
        assert_eq!(state(&config)["bar_processor_kind"], json!("stacked"));
        assert!(
            set(&mut config, "bar_processor_kind", &json!("cores"))
                .unwrap()
                .is_ok()
        );
        assert_eq!(config.bar_processor_kind, Kind::Cores);
        assert_eq!(state(&config)["bar_processor_kind"], json!("cores"));
        for path in [
            "bar_memory_kind",
            "bar_network_kind",
            "bar_gpu_kind",
            "bar_heat_kind",
            "bar_disk_kind",
        ] {
            assert!(
                set(&mut config, path, &json!("cores")).unwrap().is_err(),
                "{path}"
            );
            assert!(
                set(&mut config, path, &json!("graph")).unwrap().is_ok(),
                "{path}"
            );
        }
        for bad in [json!("Cores"), json!(1), json!(null), json!(["text"])] {
            assert!(
                set(&mut config, "bar_disk_kind", &bad).unwrap().is_err(),
                "{bad}"
            );
        }
        assert_eq!(
            config.bar_disk_kind,
            Kind::Graph,
            "a refused kind changes nothing"
        );
        // A file that asks for the cores of the network gets the default kind.
        let hostile: Config =
            serde_json::from_value(json!({ "bar_network_kind": "cores" })).unwrap();
        assert_eq!(hostile.sanitized().bar_network_kind, Kind::Stacked);
        // A kind of a later version is read as the default instead of making the whole file unreadable, but the
        // page cannot ask for it.
        let later: Config = serde_json::from_value(json!({ "bar_gpu_kind": "pie" })).unwrap();
        assert_eq!(later.bar_gpu_kind, Kind::Stacked);
    }

    #[test]
    fn only_an_offered_interval_is_taken() {
        let mut config = Config::default();
        for seconds in SECONDS {
            assert!(
                set(&mut config, "seconds", &json!(seconds))
                    .unwrap()
                    .is_ok()
            );
            assert_eq!(config.seconds, seconds);
        }
        for bad in [
            json!(3),
            json!(0),
            json!(-1),
            json!(2.5),
            json!("2"),
            json!(null),
            json!(1e12),
        ] {
            assert!(set(&mut config, "seconds", &bad).unwrap().is_err(), "{bad}");
        }
        assert_eq!(config.seconds, 5, "a refused value changes nothing");
    }
}
