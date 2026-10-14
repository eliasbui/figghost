use crate::Kind;

const FOLD_GROUPS: &[(char, &str)] = &[
    ('a', "àáạảãâầấậẩẫăằắặẳẵäåā"),
    ('e', "èéẹẻẽêềếệểễëēęě"),
    ('i', "ìíịỉĩïīîį"),
    ('o', "òóọỏõôồốộổỗơờớợởỡöøō"),
    ('u', "ùúụủũưừứựửữüūûů"),
    ('y', "ỳýỵỷỹÿ"),
    ('d', "đ"),
    ('c', "çćč"),
    ('n', "ñń"),
    ('s', "śš"),
    ('z', "źżž"),
];

/// Lowercases and strips Vietnamese and common Latin diacritics so "Nguyễn" matches "nguyen".
/// Decomposed input (macOS file names are NFD) loses its combining marks too.
pub fn fold(s: &str) -> String {
    s.chars()
        .flat_map(char::to_lowercase)
        .filter(|c| !('\u{0300}'..='\u{036F}').contains(c))
        .map(|c| {
            FOLD_GROUPS
                .iter()
                .find(|(_, set)| set.contains(c))
                .map_or(c, |(base, _)| *base)
        })
        .collect()
}

/// Greedy in-order subsequence match over already folded input; `None` when it does not match.
pub fn fuzzy_score(needle: &str, hay: &str) -> Option<i32> {
    const WINDOW: usize = 600;
    let hay: Vec<char> = hay.chars().take(WINDOW).collect();
    let (mut score, mut pos, mut last) = (0i32, 0usize, None::<usize>);
    for n in needle.chars().filter(|c| !c.is_whitespace()) {
        let skipped = hay.get(pos..)?.iter().position(|&c| c == n)?;
        let at = pos + skipped;
        score += 1;
        if last.is_some_and(|l| at == l + 1) {
            score += 4;
        }
        if at == 0 || !hay[at - 1].is_alphanumeric() {
            score += 3;
        }
        score -= (skipped.min(8) / 2) as i32;
        last = Some(at);
        pos = at + 1;
    }
    Some(score)
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Query {
    pub kind: Option<Kind>,
    pub pinned_only: bool,
    /// Folded search terms, whitespace separated in the raw input.
    pub terms: Vec<String>,
}

const FILTER_NAMES: [&str; 6] = ["text", "link", "code", "image", "file", "pin"];

/// Splits `@image`, `@link`, `@code`, `@text`, `@file`, `@pin` filters from free-text terms.
/// A half-typed filter name such as `@ima` is ignored so the list does not flicker while typing.
pub fn parse_query(raw: &str) -> Query {
    let mut q = Query::default();
    for token in raw.split_whitespace() {
        let Some(name) = token.strip_prefix('@') else {
            q.terms.push(fold(token));
            continue;
        };
        match name.to_lowercase().as_str() {
            "text" => q.kind = Some(Kind::Text),
            "link" | "url" => q.kind = Some(Kind::Link),
            "code" => q.kind = Some(Kind::Code),
            "image" | "img" => q.kind = Some(Kind::Image),
            "file" => q.kind = Some(Kind::File),
            "pin" | "pinned" => q.pinned_only = true,
            partial if FILTER_NAMES.iter().any(|f| f.starts_with(partial)) => {}
            _ => q.terms.push(fold(token)),
        }
    }
    q
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fold_handles_vietnamese_and_decomposed_forms() {
        assert_eq!(fold("Nguyễn Văn Đức"), "nguyen van duc");
        assert_eq!(fold("Ph\u{1ea1}m"), "pham");
        // "Nguyễn" typed as base letters plus combining marks, as Finder returns it.
        assert_eq!(fold("Nguye\u{0302}\u{0303}n"), "nguyen");
        assert_eq!(fold("Crème Brûlée"), "creme brulee");
    }

    #[test]
    fn fuzzy_requires_order_and_prefers_tight_matches() {
        assert_eq!(fuzzy_score("xyz", "abc"), None);
        assert_eq!(fuzzy_score("cba", "abc"), None);
        let tight = fuzzy_score("clip", "clipboard manager").unwrap();
        let loose = fuzzy_score("clip", "c l i p scattered").unwrap();
        assert!(tight > loose, "{tight} vs {loose}");
    }

    #[test]
    fn parse_query_extracts_filters() {
        let q = parse_query("@image logo");
        assert_eq!(
            (q.kind, q.terms.as_slice()),
            (Some(Kind::Image), &["logo".to_string()][..])
        );
        assert!(parse_query("@pin").pinned_only);
        assert_eq!(parse_query("@ima"), Query::default());
        assert_eq!(parse_query("@nothing x").terms, ["@nothing", "x"]);
        assert_eq!(parse_query("Đức").terms, ["duc"]);
    }
}
