//! Embedded UI strings. Add a key to every catalog to translate new text.

use std::collections::HashMap;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU8, Ordering};

type Strings = HashMap<String, String>;

static ENGLISH: OnceLock<Strings> = OnceLock::new();
static SERBIAN: OnceLock<Strings> = OnceLock::new();
static FRENCH: OnceLock<Strings> = OnceLock::new();
static SPANISH: OnceLock<Strings> = OnceLock::new();
static GREEK: OnceLock<Strings> = OnceLock::new();
static GERMAN: OnceLock<Strings> = OnceLock::new();
static LANGUAGE: AtomicU8 = AtomicU8::new(0);

pub const LANGUAGES: [&str; 6] = ["en", "sr", "fr", "es", "el", "de"];
pub const LANGUAGE_NAMES: [&str; 6] = [
    "English",
    "Српски (ћирилица)",
    "Français",
    "Español",
    "Ελληνικά",
    "Deutsch",
];

pub fn language_index(value: &str) -> usize {
    LANGUAGES
        .iter()
        .position(|code| *code == value)
        .unwrap_or(0)
}

fn english() -> &'static Strings {
    ENGLISH.get_or_init(|| {
        serde_json::from_str(include_str!("../locales/en.json"))
            .expect("English translation catalog must be valid JSON")
    })
}

fn serbian() -> &'static Strings {
    SERBIAN.get_or_init(|| {
        serde_json::from_str(include_str!("../locales/sr.json"))
            .expect("Serbian translation catalog must be valid JSON")
    })
}

fn french() -> &'static Strings {
    FRENCH.get_or_init(|| {
        serde_json::from_str(include_str!("../locales/fr.json")).expect("valid French catalog")
    })
}

fn spanish() -> &'static Strings {
    SPANISH.get_or_init(|| {
        serde_json::from_str(include_str!("../locales/es.json")).expect("valid Spanish catalog")
    })
}

fn greek() -> &'static Strings {
    GREEK.get_or_init(|| {
        serde_json::from_str(include_str!("../locales/el.json")).expect("valid Greek catalog")
    })
}

fn german() -> &'static Strings {
    GERMAN.get_or_init(|| {
        serde_json::from_str(include_str!("../locales/de.json")).expect("valid German catalog")
    })
}

/// The active language for the desktop UI.
pub fn language() -> &'static str {
    LANGUAGES[LANGUAGE.load(Ordering::Relaxed) as usize]
}

pub fn set_language(value: &str) {
    LANGUAGE.store(language_index(value) as u8, Ordering::Relaxed);
}

/// Return a translated string, falling back to English for missing keys.
pub fn tr(key: &str) -> &'static str {
    let selected = match language() {
        "sr" => Some(serbian()),
        "fr" => Some(french()),
        "es" => Some(spanish()),
        "el" => Some(greek()),
        "de" => Some(german()),
        _ => None,
    };
    if let Some(value) = selected.and_then(|catalog| catalog.get(key)) {
        return value;
    }
    english()
        .get(key)
        .map(String::as_str)
        .unwrap_or_else(|| panic!("missing English translation key: {key}"))
}

/// Replace named placeholders such as `{count}` in a translated sentence.
pub fn tr_args(key: &str, args: &[(&str, &str)]) -> String {
    let mut result = tr(key).to_owned();
    for (name, value) in args {
        result = result.replace(&format!("{{{name}}}"), value);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn placeholders(value: &str) -> BTreeSet<&str> {
        value
            .split('{')
            .skip(1)
            .filter_map(|part| part.split_once('}').map(|(name, _)| name))
            .collect()
    }

    #[test]
    fn catalogs_have_matching_keys_and_placeholders() {
        let en = english();
        for (code, catalog) in [
            ("sr", serbian()),
            ("fr", french()),
            ("es", spanish()),
            ("el", greek()),
            ("de", german()),
        ] {
            assert_eq!(
                en.keys().collect::<BTreeSet<_>>(),
                catalog.keys().collect(),
                "language: {code}"
            );
            for (key, english) in en {
                let translated = &catalog[key];
                assert!(!english.trim().is_empty(), "empty English string: {key}");
                assert!(!translated.trim().is_empty(), "empty {code} string: {key}");
                assert_eq!(
                    placeholders(english),
                    placeholders(translated),
                    "language: {code}, key: {key}"
                );
            }
        }
    }

    #[test]
    fn named_placeholders_are_replaced() {
        let result = tr_args("onboarding.found_one", &[("count", "3")]);
        assert!(!result.contains("{count}"));
        assert!(result.contains('3'));
    }

    #[test]
    fn source_lookups_exist_in_english_catalog() {
        let sources = [
            include_str!("main.rs"),
            include_str!("app.rs"),
            include_str!("platform/mac.rs"),
            include_str!("ui/main_view.rs"),
            include_str!("ui/modals.rs"),
            include_str!("ui/onboarding.rs"),
            include_str!("ui/version_manager.rs"),
            include_str!("i18n.rs"),
        ];
        for source in sources {
            for marker in ["tr(\"", "tr_args(\""] {
                for (start, _) in source.match_indices(marker) {
                    if start > 0 && source.as_bytes()[start - 1].is_ascii_alphanumeric() {
                        continue;
                    }
                    let key = source[start + marker.len()..].split('"').next().unwrap();
                    assert!(english().contains_key(key), "missing catalog key: {key}");
                }
            }
        }
    }
}
