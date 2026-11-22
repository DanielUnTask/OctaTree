//! Node naming: turns file and folder names into node names.
//!
//! Pure functions, no I/O.

/// Splits a name into words: a boundary before every ASCII capital letter and
/// at runs of whitespace, `_` and `-`. Empty words are dropped.
pub fn split_words(s: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut word = String::new();
    for c in s.chars() {
        if c.is_ascii_uppercase() {
            if !word.is_empty() {
                words.push(std::mem::take(&mut word));
            }
            word.push(c);
        } else if c == '_' || c == '-' || c.is_whitespace() {
            if !word.is_empty() {
                words.push(std::mem::take(&mut word));
            }
        } else {
            word.push(c);
        }
    }
    if !word.is_empty() {
        words.push(word);
    }
    words
}

/// Capitalizes a word; words of at most `short_max` UTF-16 units are fully
/// uppercased instead.
fn push_pascal_word(word: &str, short_max: usize, out: &mut String) {
    if word.encode_utf16().count() <= short_max {
        out.extend(word.chars().flat_map(char::to_uppercase));
    } else if let Some(first) = word.chars().next() {
        out.extend(first.to_uppercase());
        out.push_str(&word[first.len_utf8()..].to_lowercase());
    }
}

/// PascalCase with a configurable short-word threshold.
pub fn pascal_with(s: &str, short_max: usize) -> String {
    let mut out = String::with_capacity(s.len());
    for w in split_words(s) {
        push_pascal_word(&w, short_max, &mut out);
    }
    out
}

/// Output casing style for node names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaseStyle {
    /// `MyModule`
    #[default]
    Pascal,
    /// `myModule`
    Camel,
    /// `my_module`
    Snake,
    /// Leave names untouched.
    Keep,
}

/// How `keep_prefixes` are matched.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrefixMatch {
    /// Prefix must be followed by an uppercase letter, digit, separator or end.
    #[default]
    Word,
    /// Plain "starts with" (`use` also keeps `user`).
    StartsWith,
}

/// Naming configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamingRules {
    /// Casing applied to names that are not kept verbatim.
    pub case: CaseStyle,
    /// Names starting with one of these keep their casing.
    pub keep_prefixes: Vec<String>,
    /// Prefix matching mode.
    pub prefix_match: PrefixMatch,
    /// Words with at most this many characters are fully uppercased (pascal/camel).
    pub short_word_max: usize,
}

impl Default for NamingRules {
    fn default() -> Self {
        Self {
            case: CaseStyle::Pascal,
            keep_prefixes: Vec::new(),
            prefix_match: PrefixMatch::Word,
            short_word_max: 2,
        }
    }
}

impl NamingRules {
    fn keeps(&self, stem: &str) -> bool {
        self.keep_prefixes.iter().any(|p| match self.prefix_match {
            PrefixMatch::StartsWith => stem.starts_with(p.as_str()),
            PrefixMatch::Word => {
                !p.is_empty()
                    && stem.strip_prefix(p.as_str()).is_some_and(|rest| {
                        rest.chars().next().is_none_or(|c| {
                            c.is_uppercase()
                                || c.is_ascii_digit()
                                || matches!(c, '_' | '-' | '.')
                                || c.is_whitespace()
                        })
                    })
            }
        })
    }

    /// Converts a file/folder stem into a node name.
    pub fn apply(&self, stem: &str) -> String {
        if self.case == CaseStyle::Keep || self.keeps(stem) {
            return stem.to_string();
        }
        match self.case {
            CaseStyle::Pascal => pascal_with(stem, self.short_word_max),
            CaseStyle::Camel => {
                let mut words = split_words(stem).into_iter();
                let mut out = String::with_capacity(stem.len());
                if let Some(first) = words.next() {
                    out.push_str(&first.to_lowercase());
                }
                for w in words {
                    push_pascal_word(&w, self.short_word_max, &mut out);
                }
                out
            }
            CaseStyle::Snake => split_words(stem)
                .iter()
                .map(|w| w.to_lowercase())
                .collect::<Vec<_>>()
                .join("_"),
            CaseStyle::Keep => stem.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pascal_table() {
        let cases = [
            ("DamageMath", "DamageMath"),
            ("HUD", "HUD"),
            ("cool_module", "CoolModule"),
            ("my_cool-module", "MYCoolModule"),
            ("id", "ID"),
            ("ab", "AB"),
            ("abc", "Abc"),
            ("fooBar", "FooBar"),
            ("  spaced  out ", "SpacedOut"),
            ("", ""),
        ];
        for (input, expected) in cases {
            assert_eq!(pascal_with(input, 2), expected, "input {input:?}");
        }
    }

    #[test]
    fn styles_and_word_boundary_prefixes() {
        let rules = NamingRules {
            keep_prefixes: vec!["use".into(), "do".into()],
            ..NamingRules::default()
        };
        assert_eq!(rules.apply("useThing"), "useThing");
        assert_eq!(rules.apply("use"), "use");
        assert_eq!(rules.apply("use_x"), "use_x");
        assert_eq!(rules.apply("user_data"), "UserData");
        assert_eq!(rules.apply("document"), "Document");

        let starts = NamingRules {
            prefix_match: PrefixMatch::StartsWith,
            ..rules
        };
        assert_eq!(starts.apply("user_data"), "user_data");

        let camel = NamingRules {
            case: CaseStyle::Camel,
            ..NamingRules::default()
        };
        assert_eq!(camel.apply("damage_math"), "damageMath");
        let snake = NamingRules {
            case: CaseStyle::Snake,
            ..NamingRules::default()
        };
        assert_eq!(snake.apply("DamageMath"), "damage_math");
        let keep = NamingRules {
            case: CaseStyle::Keep,
            ..NamingRules::default()
        };
        assert_eq!(keep.apply("weird_Name"), "weird_Name");
        let no_short = NamingRules {
            short_word_max: 0,
            ..NamingRules::default()
        };
        assert_eq!(no_short.apply("ui_kit"), "UiKit");
    }
}
