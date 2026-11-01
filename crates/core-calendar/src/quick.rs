//! A reminder written the way a person says it, in English or Vietnamese: "call mom tomorrow 9h", "nop bao cao thu 6 5pm",
//! "tonight water the plants", "in 30 minutes take the bread out". The words that name a day or a time are taken out of the
//! title and become when it is due. Pure: the instant it is now and the local time zone are passed in.

use crate::format::{Offset, day_index, day_start};

const DAY: i64 = 86_400;

/// When a reminder is due, and whether it has no time of day.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Due {
    pub at: i64,
    pub date_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Quick {
    pub title: String,
    pub due: Option<Due>,
}

/// 0 is Sunday.
fn weekday(day: i64) -> i64 {
    (day + 4).rem_euclid(7)
}

/// A word without its case, its accents and the marks around it, so that `Tối` and `toi,` are the same word.
fn plain(word: &str) -> String {
    word.trim_matches(|c: char| !c.is_alphanumeric())
        .chars()
        .map(|c| match c {
            '\u{111}' | '\u{110}' => 'd',
            c => c,
        })
        .flat_map(char::to_lowercase)
        .map(strip_accent)
        .collect()
}

fn strip_accent(c: char) -> char {
    match c {
        'à' | 'á' | 'ả' | 'ã' | 'ạ' | 'â' | 'ầ' | 'ấ' | 'ẩ' | 'ẫ' | 'ậ' | 'ă' | 'ằ' | 'ắ' | 'ẳ'
        | 'ẵ' | 'ặ' => 'a',
        'è' | 'é' | 'ẻ' | 'ẽ' | 'ẹ' | 'ê' | 'ề' | 'ế' | 'ể' | 'ễ' | 'ệ' => 'e',
        'ì' | 'í' | 'ỉ' | 'ĩ' | 'ị' => 'i',
        'ò' | 'ó' | 'ỏ' | 'õ' | 'ọ' | 'ô' | 'ồ' | 'ố' | 'ổ' | 'ỗ' | 'ộ' | 'ơ' | 'ờ' | 'ớ' | 'ở'
        | 'ỡ' | 'ợ' => 'o',
        'ù' | 'ú' | 'ủ' | 'ũ' | 'ụ' | 'ư' | 'ừ' | 'ứ' | 'ử' | 'ữ' | 'ự' => 'u',
        'ỳ' | 'ý' | 'ỷ' | 'ỹ' | 'ỵ' => 'y',
        c => c,
    }
}

/// A clock time as it is written, with the part of the day that may follow: `9h`, `9h30`, `9:30`, `9am`, `5:15pm`.
/// Returns the hour and minute, and whether the writing already said which half of the day.
fn clock(word: &str) -> Option<(u32, u32, bool)> {
    let w = plain(word);
    let (digits, meridiem) = if let Some(rest) = w.strip_suffix("am") {
        (rest, Some(false))
    } else if let Some(rest) = w.strip_suffix("pm") {
        (rest, Some(true))
    } else {
        (w.as_str(), None)
    };
    let (hour, minute) = if let Some((h, m)) = digits.split_once('h') {
        (h, if m.is_empty() { "0" } else { m })
    } else if let Some((h, m)) = digits.split_once(':') {
        (h, m)
    } else if meridiem.is_some() {
        (digits, "0")
    } else {
        return None;
    };
    let (hour, minute): (u32, u32) = (hour.parse().ok()?, minute.parse().ok()?);
    if minute > 59 || hour > 23 || (meridiem.is_some() && !(1..=12).contains(&hour)) {
        return None;
    }
    let hour = match meridiem {
        Some(true) => hour % 12 + 12,
        Some(false) => hour % 12,
        None => hour,
    };
    Some((hour, minute, meridiem.is_some()))
}

/// Which half of the day a Vietnamese word names: `sang` is morning, `chieu` and `toi` are afternoon and evening.
fn half(word: &str) -> Option<bool> {
    match plain(word).as_str() {
        "sang" => Some(false),
        "chieu" | "toi" | "dem" => Some(true),
        _ => None,
    }
}

/// The weekday a word names, 0 being Sunday.
fn weekday_of(word: &str) -> Option<i64> {
    Some(match plain(word).as_str() {
        "sunday" | "sun" | "cn" => 0,
        "monday" | "mon" | "t2" => 1,
        "tuesday" | "tue" | "t3" => 2,
        "wednesday" | "wed" | "t4" => 3,
        "thursday" | "thu" | "t5" => 4,
        "friday" | "fri" | "t6" => 5,
        "saturday" | "sat" | "t7" => 6,
        _ => return None,
    })
}

/// A word that only joins what comes after it to the time: it goes with the time, not into the title.
fn connective(word: &str) -> bool {
    matches!(
        plain(word).as_str(),
        "at" | "on" | "by" | "vao" | "luc" | "ngay" | "den" | "truoc"
    )
}

pub fn parse(text: &str, now: i64, offset: Offset) -> Quick {
    let words: Vec<&str> = text.split_whitespace().collect();
    let today = day_index(now, offset);
    let mut day: Option<i64> = None;
    let mut time: Option<(u32, u32, bool)> = None;
    let mut relative: Option<i64> = None;
    let mut taken = vec![false; words.len()];
    let mut i = 0;
    while i < words.len() {
        let w = plain(words[i]);
        let next = words.get(i + 1).map(|n| plain(n));
        let take = |taken: &mut Vec<bool>, from: usize, count: usize| {
            for t in taken.iter_mut().skip(from).take(count) {
                *t = true;
            }
        };
        // "in 30 minutes", "sau 2 gio"
        if (w == "in" || w == "sau")
            && let Some(n) = next.as_deref().and_then(|n| n.parse::<i64>().ok())
        {
            let unit = words.get(i + 2).map(|u| plain(u)).unwrap_or_default();
            let seconds = match unit.as_str() {
                "min" | "mins" | "minute" | "minutes" | "phut" => Some(60),
                "h" | "hr" | "hrs" | "hour" | "hours" | "gio" | "tieng" => Some(3600),
                "day" | "days" | "ngay" => Some(DAY),
                _ => None,
            };
            if let Some(unit) = seconds
                && (1..=1000).contains(&n)
            {
                relative = Some(n * unit);
                take(&mut taken, i, 3);
                i += 3;
                continue;
            }
        }
        if matches!(w.as_str(), "today" | "hnay") || (w == "hom" && next.as_deref() == Some("nay"))
        {
            day = Some(today);
            take(&mut taken, i, if w == "hom" { 2 } else { 1 });
        } else if matches!(w.as_str(), "tonight") || (w == "toi" && next.as_deref() == Some("nay"))
        {
            day = Some(today);
            time.get_or_insert((19, 0, true));
            take(&mut taken, i, if w == "toi" { 2 } else { 1 });
        } else if matches!(w.as_str(), "tomorrow" | "tmr" | "tmrw" | "mai")
            || (w == "ngay" && next.as_deref() == Some("mai"))
        {
            day = Some(today + 1);
            take(&mut taken, i, if w == "ngay" { 2 } else { 1 });
        } else if w == "ngay" && next.as_deref() == Some("kia") {
            day = Some(today + 2);
            take(&mut taken, i, 2);
        } else if w == "thu"
            && let Some(d) = next.as_deref().and_then(|n| match n {
                "hai" | "2" => Some(1),
                "ba" | "3" => Some(2),
                "tu" | "4" => Some(3),
                "nam" | "5" => Some(4),
                "sau" | "6" => Some(5),
                "bay" | "7" => Some(6),
                _ => None,
            })
        {
            day = Some(
                today
                    + (d - weekday(today)).rem_euclid(7)
                    + if d == weekday(today) { 7 } else { 0 },
            );
            take(&mut taken, i, 2);
        } else if let Some(d) = weekday_of(words[i]) {
            day = Some(
                today
                    + (d - weekday(today)).rem_euclid(7).max(0)
                    + if d == weekday(today) { 7 } else { 0 },
            );
            take(&mut taken, i, 1);
        } else if w == "chu" && next.as_deref() == Some("nhat") {
            day = Some(today + (7 - weekday(today)) % 7 + if weekday(today) == 0 { 7 } else { 0 });
            take(&mut taken, i, 2);
        } else if w == "noon" || w == "trua" {
            time = Some((12, 0, true));
            take(&mut taken, i, 1);
        } else if let Some(h) = w.parse::<u32>().ok().filter(|h| (1..=12).contains(h))
            && let Some(pm) = words.get(i + 1).and_then(|n| half(n))
        {
            time = Some((if pm && h < 12 { h + 12 } else { h % 12 }, 0, true));
            take(&mut taken, i, 2);
            i += 1;
        } else if let Some(c) = clock(words[i]) {
            time = Some(c);
            take(&mut taken, i, 1);
            // "7h toi", "5 chieu": the part of the day after the number.
            if let Some(pm) = words.get(i + 1).and_then(|n| half(n)) {
                let (h, m, _) = c;
                time = Some((if pm && h < 12 { h + 12 } else { h }, m, true));
                take(&mut taken, i + 1, 1);
                i += 1;
            }
        }
        i += 1;
    }
    // A connective goes when what it joins went: "at 5pm", "luc 9h".
    for k in 0..words.len() {
        if !taken[k] && connective(words[k]) && taken.get(k + 1).copied().unwrap_or(false) {
            taken[k] = true;
        }
    }
    let title: String = words
        .iter()
        .enumerate()
        .filter(|(k, _)| !taken[*k])
        .map(|(_, w)| *w)
        .collect::<Vec<_>>()
        .join(" ");
    let title = if title.trim().is_empty() {
        text.trim().to_owned()
    } else {
        title
    };

    let due = if let Some(seconds) = relative {
        Some(Due {
            at: now + seconds,
            date_only: false,
        })
    } else {
        match (day, time) {
            (None, None) => None,
            (Some(d), None) => Some(Due {
                at: day_start(d, offset),
                date_only: true,
            }),
            (d, Some((h, m, _))) => {
                let seconds = i64::from(h) * 3600 + i64::from(m) * 60;
                let at_day = |d: i64| day_start(d, offset) + seconds;
                // A time with no day is today when it is still to come, and tomorrow when it has gone.
                let d = d.unwrap_or(if at_day(today) > now {
                    today
                } else {
                    today + 1
                });
                Some(Due {
                    at: at_day(d),
                    date_only: false,
                })
            }
        }
    };
    Quick { title, due }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Wednesday 2025-11-05, 10:00 in a zone 7 hours ahead of UTC.
    const OFFSET: i64 = 7 * 3600;
    const NOW: i64 = 1_762_311_600; // 2025-11-05 03:00:00 UTC
    fn zone(_: i64) -> i32 {
        OFFSET as i32
    }
    fn at(day_offset: i64, hour: i64, minute: i64) -> i64 {
        day_start(day_index(NOW, &zone) + day_offset, &zone) + hour * 3600 + minute * 60
    }
    fn q(text: &str) -> Quick {
        parse(text, NOW, &zone)
    }

    #[test]
    fn the_local_day_of_the_test_is_the_one_the_comments_say() {
        assert_eq!(weekday(day_index(NOW, &zone)), 3, "a Wednesday");
        assert_eq!(at(0, 10, 0), NOW);
    }

    #[test]
    fn a_day_and_a_time_in_english_are_taken_out_of_the_title() {
        let r = q("call mom tomorrow at 9am");
        assert_eq!(r.title, "call mom");
        assert_eq!(
            r.due,
            Some(Due {
                at: at(1, 9, 0),
                date_only: false
            })
        );
        let r = q("pay rent friday 5:30pm");
        assert_eq!(
            (r.title.as_str(), r.due),
            (
                "pay rent",
                Some(Due {
                    at: at(2, 17, 30),
                    date_only: false
                })
            )
        );
        let r = q("renew passport today");
        assert_eq!(
            (r.title.as_str(), r.due),
            (
                "renew passport",
                Some(Due {
                    at: at(0, 0, 0),
                    date_only: true
                })
            )
        );
    }

    #[test]
    fn vietnamese_days_and_hours_are_understood_with_or_without_accents() {
        let r = q("n\u{1ed9}p b\u{e1}o c\u{e1}o th\u{1ee9} 6 l\u{fa}c 5 chi\u{1ec1}u");
        assert_eq!(r.title, "n\u{1ed9}p b\u{e1}o c\u{e1}o", "{r:?}");
        assert_eq!(
            r.due,
            Some(Due {
                at: at(2, 17, 0),
                date_only: false
            })
        );
        let r = q("goi dien cho me ngay mai 9h30");
        assert_eq!(
            (r.title.as_str(), r.due),
            (
                "goi dien cho me",
                Some(Due {
                    at: at(1, 9, 30),
                    date_only: false
                })
            )
        );
        let r = q("t\u{1ed1}i nay t\u{1ec9}a hoa");
        assert_eq!(r.title, "t\u{1ec9}a hoa");
        assert_eq!(
            r.due,
            Some(Due {
                at: at(0, 19, 0),
                date_only: false
            }),
            "tonight is seven in the evening"
        );
        let r = q("mua sua h\u{f4}m nay");
        assert_eq!(
            (r.title.as_str(), r.due.map(|d| d.date_only)),
            ("mua sua", Some(true))
        );
        let r = q("hop ch\u{1ee7} nh\u{1ead}t 8h s\u{e1}ng");
        assert_eq!(
            r.due,
            Some(Due {
                at: at(4, 8, 0),
                date_only: false
            }),
            "{r:?}"
        );
    }

    #[test]
    fn a_time_alone_is_today_if_it_is_to_come_and_tomorrow_if_it_has_gone() {
        assert_eq!(
            q("stand up 11:00").due,
            Some(Due {
                at: at(0, 11, 0),
                date_only: false
            })
        );
        assert_eq!(
            q("stand up 9:00").due,
            Some(Due {
                at: at(1, 9, 0),
                date_only: false
            })
        );
        assert_eq!(
            q("lunch noon").due,
            Some(Due {
                at: at(0, 12, 0),
                date_only: false
            })
        );
    }

    #[test]
    fn a_weekday_is_the_next_one_and_the_same_weekday_means_next_week() {
        assert_eq!(
            q("gym wednesday").due,
            Some(Due {
                at: at(7, 0, 0),
                date_only: true
            })
        );
        assert_eq!(
            q("gym thursday").due,
            Some(Due {
                at: at(1, 0, 0),
                date_only: true
            })
        );
        assert_eq!(
            q("gym monday").due,
            Some(Due {
                at: at(5, 0, 0),
                date_only: true
            })
        );
        assert_eq!(
            q("gym sunday").due,
            Some(Due {
                at: at(4, 0, 0),
                date_only: true
            })
        );
    }

    #[test]
    fn a_wait_counts_from_now() {
        assert_eq!(
            q("take the bread out in 30 minutes").due,
            Some(Due {
                at: NOW + 1800,
                date_only: false
            })
        );
        assert_eq!(
            q("take the bread out in 30 minutes").title,
            "take the bread out"
        );
        assert_eq!(
            q("sau 2 gi\u{1edd} nh\u{1eaf}c u\u{1ed1}ng thu\u{1ed1}c").due,
            Some(Due {
                at: NOW + 7200,
                date_only: false
            })
        );
        assert_eq!(
            q("in 2 days renew").due,
            Some(Due {
                at: NOW + 2 * DAY,
                date_only: false
            })
        );
    }

    #[test]
    fn what_names_no_time_stays_whole_and_nonsense_gives_no_date() {
        let r = q("buy milk");
        assert_eq!((r.title.as_str(), r.due), ("buy milk", None));
        // "mai" is a Vietnamese word for tomorrow, but "Mai" as a name in a sentence is still read so: the person can say
        // "g\u{1ecd}i Mai" and edit the date. This is the documented limit of a parser without a dictionary.
        assert!(q("gọi Mai").due.is_some());
        let r = q("tomorrow");
        assert_eq!(
            r.title, "tomorrow",
            "a title cannot be empty: the whole text stands in"
        );
        assert_eq!(
            r.due,
            Some(Due {
                at: at(1, 0, 0),
                date_only: true
            })
        );
        assert_eq!(q("in 0 minutes x").due, None);
        assert_eq!(q("25:99 x").due, None);
        assert_eq!(q("13pm x").due, None);
        assert_eq!(q("  ").title, "");
    }

    #[test]
    fn a_time_with_a_half_of_the_day_after_it_is_moved_into_the_afternoon() {
        assert_eq!(
            q("rehearse 7h t\u{1ed1}i").due,
            Some(Due {
                at: at(0, 19, 0),
                date_only: false
            })
        );
        assert_eq!(
            q("rehearse 12h trua").due,
            Some(Due {
                at: at(0, 12, 0),
                date_only: false
            })
        );
    }
}
