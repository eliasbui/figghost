//! File names for saved pictures.

/// A local date and time, as the clock on the wall shows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stamp {
    pub year: i32,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
}

/// The name of the `attempt`-th try (0 is the first) at saving at `stamp`: `Figghost 2026-10-15 at 14.03.22.png`,
/// then `... 2.png`, `... 3.png`. The caller creates the file without overwriting and moves on to the next
/// attempt when the name is taken. No character in it is one a file system refuses.
pub fn file_name(stamp: Stamp, attempt: u32) -> String {
    let base = format!(
        "Figghost {:04}-{:02}-{:02} at {:02}.{:02}.{:02}",
        stamp.year, stamp.month, stamp.day, stamp.hour, stamp.minute, stamp.second
    );
    if attempt == 0 {
        format!("{base}.png")
    } else {
        format!("{base} {}.png", attempt + 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const STAMP: Stamp = Stamp {
        year: 2026,
        month: 3,
        day: 9,
        hour: 7,
        minute: 5,
        second: 1,
    };

    #[test]
    fn the_first_name_is_the_time_and_the_next_ones_are_numbered() {
        assert_eq!(file_name(STAMP, 0), "Figghost 2026-03-09 at 07.05.01.png");
        assert_eq!(file_name(STAMP, 1), "Figghost 2026-03-09 at 07.05.01 2.png");
        assert_eq!(file_name(STAMP, 8), "Figghost 2026-03-09 at 07.05.01 9.png");
    }

    #[test]
    fn names_are_unique_per_attempt_and_have_no_separator() {
        let names: std::collections::HashSet<String> =
            (0..50).map(|n| file_name(STAMP, n)).collect();
        assert_eq!(names.len(), 50);
        assert!(
            names
                .iter()
                .all(|n| !n.contains('/') && !n.contains(':') && n.ends_with(".png"))
        );
    }
}
