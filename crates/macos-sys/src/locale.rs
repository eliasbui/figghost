use objc2_foundation::NSLocale;

/// The user's preferred languages from System Settings, most preferred first, as BCP 47 tags such as
/// `vi-VN` or `en-US`.
pub fn preferred_languages() -> Vec<String> {
    NSLocale::preferredLanguages()
        .iter()
        .map(|tag| tag.to_string())
        .collect()
}
