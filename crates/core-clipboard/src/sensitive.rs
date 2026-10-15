//! Finds likely secrets in copied text, so the history can mask them or leave them out.
//!
//! The detector prefers a miss to a false alarm on everyday text (order numbers, phone numbers, UUIDs,
//! prose about passwords): a masked item costs the user a key press, and every rule below is a shape that
//! real credentials have.

/// What kind of secret was recognised.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Secret {
    /// A payment card number that passes the Luhn check and has a known issuer prefix and length.
    Card,
    /// A key with a well known vendor shape (cloud, source hosting, payments, chat, AI services).
    Key,
    /// A bearer token or a JSON web token.
    Token,
    /// A PEM private key block.
    PrivateKey,
    /// The value after a "password" or "secret" label.
    Password,
}

/// Text past this length is not looked at; credentials are short and a huge copy is a document.
const SCAN_BYTES: usize = 256 * 1024;

/// Returns the first kind of secret found in `text`, most specific first.
pub fn detect(text: &str) -> Option<Secret> {
    let text = cut(text);
    if has_private_key(text) {
        return Some(Secret::PrivateKey);
    }
    if let Some(found) = token_secret(text) {
        return Some(found);
    }
    if has_bearer(text) {
        return Some(Secret::Token);
    }
    if has_card(text) {
        return Some(Secret::Card);
    }
    if has_labelled_password(text) {
        return Some(Secret::Password);
    }
    None
}

fn cut(text: &str) -> &str {
    if text.len() <= SCAN_BYTES {
        return text;
    }
    let mut end = SCAN_BYTES;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

fn has_private_key(text: &str) -> bool {
    text.match_indices("-----BEGIN ").any(|(at, _)| {
        let line = text[at..].lines().next().unwrap_or("");
        line.contains("PRIVATE KEY") && line.trim_end().ends_with("-----")
    })
}

fn is_token_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.')
}

/// Runs of token characters; every other character, including `=` and quotes, ends a token.
fn tokens(text: &str) -> impl Iterator<Item = &str> {
    text.split(|c: char| !is_token_char(c))
        .filter(|t| !t.is_empty())
}

fn has_digit_and_letter(s: &str) -> bool {
    s.bytes().any(|b| b.is_ascii_digit()) && s.bytes().any(|b| b.is_ascii_alphabetic())
}

fn all_of(s: &str, ok: impl Fn(u8) -> bool) -> bool {
    s.bytes().all(ok)
}

fn token_secret(text: &str) -> Option<Secret> {
    for token in tokens(text) {
        if is_jwt(token) {
            return Some(Secret::Token);
        }
        if is_vendor_key(token) {
            return Some(Secret::Key);
        }
    }
    None
}

fn is_jwt(token: &str) -> bool {
    let mut parts = token.split('.');
    let (Some(header), Some(payload), Some(signature), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return false;
    };
    let b64url = |s: &str| all_of(s, |b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-');
    // "eyJ" is base64 of `{"`, which every JSON header and payload starts with.
    header.starts_with("eyJ")
        && payload.starts_with("eyJ")
        && header.len() >= 10
        && payload.len() >= 10
        && signature.len() >= 8
        && b64url(header)
        && b64url(payload)
        && b64url(signature)
}

fn is_vendor_key(token: &str) -> bool {
    let alnum = |b: u8| b.is_ascii_alphanumeric();
    let alnum_dash = |b: u8| b.is_ascii_alphanumeric() || b == b'_' || b == b'-';
    let after = |prefix: &str| token.strip_prefix(prefix);

    // OpenAI and Anthropic keys: a long random tail, so a hyphenated word that starts with "sk-" is not one.
    if let Some(rest) = after("sk-") {
        return rest.len() >= 32 && all_of(rest, alnum_dash) && has_digit_and_letter(rest);
    }
    for prefix in ["ghp_", "gho_", "ghu_", "ghs_", "ghr_"] {
        if let Some(rest) = after(prefix) {
            return rest.len() == 36 && all_of(rest, alnum);
        }
    }
    if let Some(rest) = after("github_pat_") {
        return rest.len() >= 22 && all_of(rest, alnum_dash);
    }
    if let Some(rest) = after("glpat-") {
        return rest.len() >= 20 && all_of(rest, alnum_dash);
    }
    if let Some(rest) = after("hf_") {
        return rest.len() >= 30 && all_of(rest, alnum) && has_digit_and_letter(rest);
    }
    if let Some(rest) = after("npm_") {
        return rest.len() == 36 && all_of(rest, alnum);
    }
    if let Some(rest) = after("AIza") {
        return rest.len() == 35 && all_of(rest, alnum_dash);
    }
    if let Some(rest) = after("SG.") {
        let mut parts = rest.split('.');
        if let (Some(id), Some(secret), None) = (parts.next(), parts.next(), parts.next()) {
            return id.len() >= 20 && secret.len() >= 40 && all_of(id, alnum_dash);
        }
        return false;
    }
    // Slack: xoxb-123-456-abc.
    if let Some(rest) = ["xoxa-", "xoxb-", "xoxp-", "xoxr-", "xoxs-"]
        .iter()
        .find_map(|p| after(p))
    {
        return rest.len() >= 10 && rest.bytes().any(|b| b.is_ascii_digit());
    }
    // Stripe: only the secret and restricted keys; publishable keys are meant to be public.
    for prefix in ["sk_live_", "sk_test_", "rk_live_", "rk_test_"] {
        if let Some(rest) = after(prefix) {
            return rest.len() >= 16 && all_of(rest, alnum);
        }
    }
    // AWS access key ids are exactly 20 upper case letters and digits behind a four letter prefix.
    const AWS: [&str; 6] = ["AKIA", "ASIA", "AGPA", "AIDA", "AROA", "ANPA"];
    token.len() == 20
        && AWS.iter().any(|p| token.starts_with(p))
        && all_of(token, |b| b.is_ascii_uppercase() || b.is_ascii_digit())
}

fn has_bearer(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower.match_indices("bearer").any(|(at, _)| {
        let before_ok = at == 0 || !lower.as_bytes()[at - 1].is_ascii_alphanumeric();
        let rest = &text[at + "bearer".len()..];
        let value = rest.trim_start_matches([' ', '\t']);
        if !before_ok || value.len() == rest.len() {
            return false;
        }
        let token: String = value
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || "._~+/=-".contains(*c))
            .collect();
        token.len() >= 20 && has_digit_and_letter(&token)
    })
}

/// Whether a digit string has the length and issuer prefix of a real card network.
fn plausible_card(digits: &[u8]) -> bool {
    let n = digits.len();
    let prefix = |len: usize| {
        digits[..len]
            .iter()
            .fold(0u32, |acc, d| acc * 10 + u32::from(*d))
    };
    let (p2, p3, p4) = (prefix(2), prefix(3), prefix(4));
    match digits[0] {
        4 => matches!(n, 13 | 16 | 19),
        5 => (51..=55).contains(&p2) && n == 16,
        2 => (2221..=2720).contains(&p4) && n == 16,
        3 => match p2 {
            34 | 37 => n == 15,
            36 | 38 | 39 => n == 14,
            30 => (300..=305).contains(&p3) && n == 14,
            35 => (3528..=3589).contains(&p4) && (16..=19).contains(&n),
            _ => false,
        },
        6 => {
            let discover = p4 == 6011 || p2 == 65 || (644..=649).contains(&p3);
            (discover || p2 == 62) && (16..=19).contains(&n)
        }
        _ => false,
    }
}

fn luhn(digits: &[u8]) -> bool {
    let sum: u32 = digits
        .iter()
        .rev()
        .enumerate()
        .map(|(i, d)| {
            let d = u32::from(*d);
            if i % 2 == 1 {
                let doubled = d * 2;
                if doubled > 9 { doubled - 9 } else { doubled }
            } else {
                d
            }
        })
        .sum();
    sum % 10 == 0
}

/// Group sizes a printed card number has; a plain run of digits has one group.
fn card_grouping(groups: &[usize]) -> bool {
    matches!(
        groups,
        [_] | [4, 4, 4, 4] | [4, 4, 4, 4, 3] | [4, 6, 5] | [4, 6, 4] | [4, 4, 4, 4, 4]
    )
}

/// Whether the byte at `at` is part of a longer identifier or version (a letter, an underscore, or a dot
/// with a word character on its other side) rather than punctuation around a number.
fn is_joined(bytes: &[u8], at: usize, other_side: Option<usize>) -> bool {
    let b = bytes[at];
    b.is_ascii_alphanumeric()
        || b == b'_'
        || (b == b'.'
            && other_side
                .and_then(|i| bytes.get(i))
                .is_some_and(u8::is_ascii_alphanumeric))
}

fn has_card(text: &str) -> bool {
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if !bytes[i].is_ascii_digit() || (i > 0 && is_joined(bytes, i - 1, i.checked_sub(2))) {
            i += 1;
            continue;
        }
        // One candidate: digit groups joined by single spaces or hyphens.
        let (mut j, mut groups, mut digits) = (i, Vec::new(), Vec::new());
        loop {
            let start = j;
            while j < bytes.len() && bytes[j].is_ascii_digit() {
                digits.push(bytes[j] - b'0');
                j += 1;
            }
            groups.push(j - start);
            let next_group = j + 1 < bytes.len()
                && matches!(bytes[j], b' ' | b'-')
                && bytes[j + 1].is_ascii_digit();
            if !next_group {
                break;
            }
            j += 1;
        }
        let end_clear = j >= bytes.len() || !is_joined(bytes, j, Some(j + 1));
        if end_clear
            && (13..=19).contains(&digits.len())
            && card_grouping(&groups)
            && digits.iter().any(|d| *d != digits[0])
            && plausible_card(&digits)
            && luhn(&digits)
        {
            return true;
        }
        // Resume after this candidate so the scan stays linear.
        i = j.max(i + 1);
    }
    false
}

/// Words that name a secret. Vietnamese is matched with and without its marks.
const LABELS: [&str; 9] = [
    "password",
    "passwd",
    "passphrase",
    "pwd",
    "secret",
    "mật khẩu",
    "mat khau",
    "mật mã",
    "mat ma",
];

/// Values that stand for "no real value here".
const PLACEHOLDERS: [&str; 15] = [
    "required",
    "optional",
    "none",
    "null",
    "nil",
    "undefined",
    "empty",
    "hidden",
    "string",
    "password",
    "secret",
    "example",
    "placeholder",
    "redacted",
    "yourpassword",
];

fn has_labelled_password(text: &str) -> bool {
    text.lines().any(line_has_labelled_password)
}

fn line_has_labelled_password(line: &str) -> bool {
    let lower = line.to_lowercase();
    // Lowercasing can change the byte length of a few scripts; the offsets below need the same text.
    if lower.len() != line.len() {
        return false;
    }
    LABELS.iter().any(|label| {
        lower.match_indices(label).any(|(at, _)| {
            // "mypassword" is another word, but "db_password" and "DB-SECRET" are labels.
            let glued = lower[..at]
                .chars()
                .next_back()
                .is_some_and(char::is_alphanumeric);
            !glued && value_after(&line[at + label.len()..])
        })
    })
}

/// Whether the text after a label holds a credible secret value.
fn value_after(rest: &str) -> bool {
    // The rest of the label word ("secret_key", "passwords") and a closing quote.
    let rest = rest.trim_start_matches(|c: char| c.is_alphanumeric() || c == '_');
    let rest = rest.trim_start_matches(['"', '\'', '`', ' ', '\t']);
    let (spoken, rest) = if let Some(r) = rest.strip_prefix(':').or_else(|| rest.strip_prefix('='))
    {
        (false, r)
    } else if let Some(r) = ["is ", "là "].iter().find_map(|w| rest.strip_prefix(w)) {
        (true, r)
    } else {
        return false;
    };
    let rest = rest.trim_start_matches([' ', '\t']);
    // `==`, `=>` and `:=` are code that compares or computes, not a literal.
    if rest.starts_with(['=', '>']) {
        return false;
    }
    let quote = rest
        .chars()
        .next()
        .filter(|c| matches!(c, '"' | '\'' | '`'));
    let value: String = match quote {
        Some(q) => rest[1..].chars().take_while(|c| *c != q).collect(),
        None => rest
            .chars()
            .take_while(|c| !c.is_whitespace() && !matches!(c, ',' | ';'))
            .collect(),
    };
    let value = value.trim_matches(['.', ')', ']']);
    if placeholder(value) || value.contains(['(', ')', '{', '}', '[', ']', '<', '>']) {
        return false;
    }
    if quote.is_none() || spoken {
        // A bare word is usually a type or a variable name, so it needs a digit or a symbol to count.
        value.chars().count() >= 6 && value.chars().any(|c| c.is_ascii_digit() || is_symbol(c))
    } else {
        value.chars().count() >= 4
    }
}

fn is_symbol(c: char) -> bool {
    c.is_ascii_punctuation() && !matches!(c, '_' | '-' | '.' | '/')
}

fn placeholder(value: &str) -> bool {
    let lower = value.to_lowercase();
    lower.is_empty()
        || PLACEHOLDERS.contains(&lower.as_str())
        || value
            .chars()
            .all(|c| matches!(c, '*' | 'x' | 'X' | '.' | '-' | '\u{2022}' | '#' | '_'))
        || value.starts_with(['$', '%'])
        || value.starts_with("{{")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kind(text: &str) -> Option<Secret> {
        detect(text)
    }

    #[test]
    fn card_numbers_with_a_valid_check_digit_are_found() {
        for number in [
            "4111111111111111",
            "4111 1111 1111 1111",
            "4111-1111-1111-1111",
            "4012888888881881",
            "5555555555554444",
            "5105 1051 0510 5100",
            "2223003122003222",
            "378282246310005",
            "3782 822463 10005",
            "371449635398431",
            "6011111111111117",
            "6011 0009 9013 9424",
            "3530111333300000",
            "30569309025904",
            "6200000000000005",
            "4222222222222",
        ] {
            assert_eq!(kind(number), Some(Secret::Card), "{number}");
        }
    }

    #[test]
    fn a_card_inside_a_sentence_is_found_and_trailing_punctuation_is_fine() {
        assert_eq!(
            kind("Pay with 4111 1111 1111 1111, expires 12/29."),
            Some(Secret::Card)
        );
        assert_eq!(kind("card: 5555555555554444."), Some(Secret::Card));
        assert_eq!(kind("(4111111111111111)"), Some(Secret::Card));
        assert_eq!(
            kind("line one\n4111111111111111\nline three"),
            Some(Secret::Card)
        );
    }

    #[test]
    fn numbers_that_only_look_like_cards_are_left_alone() {
        for text in [
            // Wrong check digit.
            "4111111111111112",
            "5555555555554445",
            "4111111111111111111",
            // Luhn valid but no known issuer.
            "7992739871003",
            "9112345678901230",
            // Order numbers, tracking numbers, timestamps, phone numbers.
            "Order #1234567890123",
            "Đơn hàng 20240115123456",
            "1700000000000",
            "+84 912 345 678",
            "+1 (415) 555-2671",
            "0912.345.678",
            "tel: 0283 822 1234",
            "2024-01-15 10:30:45",
            // Too short, too long, all one digit.
            "411111111111",
            "41111111111111111111",
            "4444444444444444",
            "0000000000000000",
            // Part of a longer identifier.
            "ID4111111111111111",
            "4111111111111111abc",
            "v4111111111111111",
            // Odd grouping.
            "41 1111 1111 1111 11",
            "4111  1111  1111  1111",
            // Identifiers with digits.
            "550e8400-e29b-41d4-a716-446655440000",
            "123e4567-e89b-12d3-a456-426614174000",
            "9780306406157",
        ] {
            assert_eq!(kind(text), None, "{text}");
        }
    }

    #[test]
    fn well_known_key_shapes_are_found() {
        let fakes = [
            "sk-proj-Ab3dEf6hIj9lMn2pQr5tUv8xYz1BcD4eFg7Hi0Jk",
            "sk-ant-api03-Xy12Zw34Vu56Ts78Rq90PoNm12LkJi34HgFe56Dc78Ba90-AbCdEfGhIj",
            "sk-abcdefghijklmnopqrstuvwxyz0123456789ABCD",
            "ghp_aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789",
            "gho_aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789",
            "github_pat_11ABCDEFG0abcdefghijkl_mnopqrstuvwxyz0123456789",
            "glpat-AbCdEfGhIjKlMnOpQrSt",
            "xoxb-1234567890-0987654321-AbCdEfGhIjKlMnOpQrStUvWx",
            "xoxp-12345-67890-abcdef",
            "sk_live_AbCdEfGhIjKlMnOpQrStUvWx",
            "sk_test_AbCdEfGhIjKlMnOpQrSt1234",
            "rk_live_AbCdEfGhIjKlMnOpQrSt1234",
            "AIzaSyA1B2C3D4E5F6G7H8I9J0KlMnOpQrStUvW",
            "AKIAIOSFODNN7EXAMPLE",
            "ASIAIOSFODNN7EXAMPLE",
            "npm_aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789",
            "hf_aBcDeFgHiJkLmNoPqRsTuVwXyZ01234567",
            "SG.AbCdEfGhIjKlMnOpQrStUv.AbCdEfGhIjKlMnOpQrStUvWxYz0123456789AbCdEfG",
        ];
        for text in fakes {
            assert_eq!(kind(text), Some(Secret::Key), "{text}");
            assert_eq!(
                kind(&format!("export API_KEY={text}\n")),
                Some(Secret::Key),
                "assigned {text}"
            );
            assert_eq!(
                kind(&format!("{{\"key\": \"{text}\"}}")),
                Some(Secret::Key),
                "quoted {text}"
            );
        }
    }

    #[test]
    fn words_and_ids_that_start_like_a_key_are_left_alone() {
        for text in [
            "sk-learn",
            "sk-learn is a library for machine learning in python",
            "sk-short",
            "task-force",
            "ghp_short",
            "ghp_aBcDeFgHiJkLmNoPqRsTuVwXyZ01234567890123456789",
            "pk_live_AbCdEfGhIjKlMnOpQrStUvWx",
            "AKIA",
            "AKIAIOSFODNN7EXAMPL",
            "AKIAIOSFODNN7EXAMPLEE",
            "AKIAiosfodnn7example",
            "AIza",
            "xoxb-short",
            "hf_hub_download",
            "npm_config_prefix",
            "glpat-short",
            "A sentence mentioning sk- and AKIA keys in general.",
        ] {
            assert_eq!(kind(text), None, "{text}");
        }
    }

    #[test]
    fn json_web_tokens_and_bearer_tokens_are_found() {
        let jwt = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4ifQ.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c";
        assert_eq!(kind(jwt), Some(Secret::Token));
        assert_eq!(
            kind(&format!("Authorization: Bearer {jwt}")),
            Some(Secret::Token)
        );
        assert_eq!(
            kind("Authorization: Bearer 7f3a9c2e1b8d4f6a0e5c3b7d9a1f2e4c"),
            Some(Secret::Token)
        );
        assert_eq!(
            kind("curl -H 'authorization: bearer ya29.a0AfH6SMBx1234567890abcdefg'"),
            Some(Secret::Token)
        );
        assert_eq!(
            kind("BEARER   abcdefghij0123456789klmnop"),
            Some(Secret::Token)
        );
    }

    #[test]
    fn text_that_mentions_tokens_without_holding_one_is_left_alone() {
        for text in [
            "Bearer of bad news",
            "the bearer token should be kept secret",
            "Bearer abc123",
            "Bearer abcdefghijklmnopqrstuvwxyz",
            "Bearer: required",
            "eyJhbGciOiJIUzI1NiJ9",
            "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxIn0",
            "a.b.c",
            "version 1.2.3",
            "www.example.com.vn",
            "file.name.with.dots.txt",
            "eyJhbGciOiJIUzI1NiJ9.notajsonpayload.signaturesignature",
        ] {
            assert_eq!(kind(text), None, "{text}");
        }
    }

    #[test]
    fn private_key_blocks_are_found() {
        for header in [
            "-----BEGIN RSA PRIVATE KEY-----",
            "-----BEGIN PRIVATE KEY-----",
            "-----BEGIN EC PRIVATE KEY-----",
            "-----BEGIN OPENSSH PRIVATE KEY-----",
            "-----BEGIN ENCRYPTED PRIVATE KEY-----",
            "-----BEGIN DSA PRIVATE KEY-----",
        ] {
            let block =
                format!("{header}\nMIIEvQIBADANBgkqhkiG9w0BAQEFAASC\n-----END PRIVATE KEY-----\n");
            assert_eq!(kind(&block), Some(Secret::PrivateKey), "{header}");
        }
        assert_eq!(
            kind("notes\n-----BEGIN RSA PRIVATE KEY-----\nabc"),
            Some(Secret::PrivateKey)
        );
    }

    #[test]
    fn public_material_is_not_a_private_key() {
        for text in [
            "-----BEGIN PUBLIC KEY-----\nMIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8A\n-----END PUBLIC KEY-----",
            "-----BEGIN CERTIFICATE-----\nMIIDdzCCAl+gAwIBAgIEAgAAuTANBgkqhkiG9w0BAQUFADBa\n-----END CERTIFICATE-----",
            "-----BEGIN CERTIFICATE REQUEST-----\nabc\n-----END CERTIFICATE REQUEST-----",
            "ssh-rsa AAAAB3NzaC1yc2EAAAADAQABAAABAQC7 user@host",
            "a private key is a secret number",
        ] {
            assert_eq!(kind(text), None, "{text}");
        }
    }

    #[test]
    fn values_after_password_and_secret_labels_are_found() {
        for text in [
            "password: hunter2",
            "Password: Tr0ub4dor&3",
            "PASSWORD=s3cr3t!pass",
            "password = \"hunter2\"",
            "password = 'correct horse'",
            "\"password\": \"hunter2\"",
            "{\"password\":\"abcd\"}",
            "DB_PASSWORD=p@ssw0rd123",
            "client_secret: 9fA3kQ7zLm",
            "SECRET = \"abcd\"",
            "secret=abc123def",
            "my password is hunter2xyz",
            "login: admin\npassword: Winter2024!\n",
            "passwd:Zx9$kLm2",
            "Mật khẩu: abc12345",
            "mat khau: Matkhau@123",
            "mật khẩu là Vn2024@abc",
            "wifi password: coffee1234",
            "Passphrase: Blue-Horses-7!",
        ] {
            assert_eq!(kind(text), Some(Secret::Password), "{text}");
        }
    }

    #[test]
    fn prose_and_code_about_passwords_are_left_alone() {
        for text in [
            "Enter your password",
            "Forgot password?",
            "password reset link sent",
            "password manager",
            "The password is required",
            "password: required",
            "Password:",
            "password: ",
            "password: ********",
            "password: \u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}",
            "password = ",
            "password: <your password>",
            "password: ${DB_PASSWORD}",
            "password: {{ vault_password }}",
            "password: string",
            "secret: String,",
            "password = input()",
            "password = get_password(user)",
            "password == other",
            "if password != confirm:",
            "password => hash(password)",
            "secret santa is on friday",
            "client_secret: null",
            "password: optional",
            "mypassword",
            "Mật khẩu: bắt buộc",
            "mật khẩu là gì",
            "set password = x",
            "passwords must contain a number",
            "password:\tabc",
        ] {
            assert_eq!(kind(text), None, "{text}");
        }
    }

    #[test]
    fn ordinary_copies_are_not_flagged() {
        for text in [
            "hello world",
            "Meeting at 3pm in room 4.12, bring the Q3 report",
            "https://example.com/path?token=short&id=42",
            "git commit -m \"fix the build\"",
            "SELECT * FROM users WHERE id = 42;",
            "fn main() {\n    println!(\"hi\");\n}",
            "Nguyễn Văn An, 0912345678, an@example.com",
            "550e8400-e29b-41d4-a716-446655440000",
            "ffffffffffffffffffffffffffffffff",
            "d41d8cd98f00b204e9800998ecf8427e",
            "da39a3ee5e6b4b0d3255bfef95601890afd80709",
            "1234567890",
            "",
            "   ",
        ] {
            assert_eq!(kind(text), None, "{text}");
        }
    }

    #[test]
    fn a_secret_among_other_text_is_still_found() {
        let text = "Notes for the deploy:\n- region: ap-southeast-1\n- use key AKIAIOSFODNN7EXAMPLE for staging\n- then run the script";
        assert_eq!(kind(text), Some(Secret::Key));
        // A private key outranks everything else in the same copy.
        let both = "password: hunter2\n-----BEGIN PRIVATE KEY-----\nabc\n-----END PRIVATE KEY-----";
        assert_eq!(kind(both), Some(Secret::PrivateKey));
    }

    #[test]
    fn multibyte_text_never_panics() {
        for text in [
            "Mật khẩu: ",
            "MẬT KHẨU: abc12345",
            "İstanbul password: abc12345",
            "pass\u{0307}word: abc12345",
            "\u{0130}\u{0130}\u{0130} secret: abc12345",
        ] {
            let _ = kind(text);
        }
        assert_eq!(kind("密码 password: abc12345"), Some(Secret::Password));
        assert_eq!(kind("MẬT KHẨU: abc12345"), Some(Secret::Password));
    }

    #[test]
    fn huge_text_is_cut_without_splitting_a_character() {
        let big = "ề".repeat(SCAN_BYTES);
        assert_eq!(kind(&big), None);
        let late = format!("{}\npassword: hunter2", "a".repeat(SCAN_BYTES));
        assert_eq!(kind(&late), None, "past the scan limit nothing is read");
    }

    #[test]
    fn the_luhn_check_matches_known_numbers() {
        let digits = |s: &str| s.bytes().map(|b| b - b'0').collect::<Vec<u8>>();
        assert!(luhn(&digits("79927398713")));
        assert!(!luhn(&digits("79927398710")));
        assert!(luhn(&digits("4111111111111111")));
    }
}
