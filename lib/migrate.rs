//! `octatree migrate <path>`: converts the routing information of an old JSON
//! config (see [`crate::import`]) into an `octatree.toml`.

use std::fmt::Write as _;

use crate::import::Imported;

fn quote(s: &str) -> String {
    // A JSON string is a valid TOML basic string.
    serde_json::to_string(s).unwrap_or_else(|_| "\"\"".into())
}

fn quote_list(items: &[String]) -> String {
    let parts: Vec<String> = items.iter().map(|s| quote(s)).collect();
    format!("[{}]", parts.join(", "))
}

/// Renders the imported routing as `octatree.toml` text.
///
/// The old format always routed names ending in `server`/`service` to the
/// server and matched prefixes with a plain "starts with", so those are
/// written out explicitly. `modules` is left empty on purpose: the old format
/// has no such setting, so the developer lists their own folders.
///
/// Rules are emitted duplicated, then no-server, then server-only, so that on
/// the same path "server-only beats no-server beats duplicated" holds, as in
/// the old format ("later rule wins" for equal paths).
pub fn to_toml(imp: &Imported) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "# Converted from an old JSON config by `octatree migrate`."
    );
    let _ = writeln!(
        out,
        "# Review it, and list your top-level folders in `modules` before generating."
    );
    let _ = writeln!(out, "version = 1");
    let _ = writeln!(out, "modules = []");
    let _ = writeln!(out, "default_to = [\"shared\"]");
    let _ = writeln!(out, "\n[containers.shared]\nmount = \"ReplicatedStorage\"");
    let _ = writeln!(
        out,
        "\n[containers.server]\nmount = \"ServerScriptService\""
    );
    let _ = writeln!(
        out,
        "\n[[suffix_rules]]\nsuffix = [\"server\", \"service\"]\nto = [\"server\"]"
    );
    let _ = writeln!(out, "\n[naming]\ncase = \"pascal\"");
    let _ = writeln!(out, "keep_prefixes = {}", quote_list(&imp.keep_prefixes));
    let _ = writeln!(out, "prefix_match = \"starts_with\"\nshort_word_max = 2");

    for p in &imp.duplicated {
        let _ = writeln!(
            out,
            "\n[[rules]]\nmatch = {}\nto = [\"shared\", \"server\"]",
            quote(p)
        );
    }
    for p in &imp.no_server {
        let _ = writeln!(out, "\n[[rules]]\nmatch = {}\nto = [\"shared\"]", quote(p));
    }
    for (p, name) in &imp.server_only {
        let _ = writeln!(out, "\n[[rules]]\nmatch = {}\nto = [\"server\"]", quote(p));
        if let Some(n) = name {
            let _ = writeln!(out, "rename = {}", quote(n));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::Settings;
    use std::path::Path;

    #[test]
    fn output_is_valid_settings_with_the_expected_rules() {
        let imp = Imported {
            server_only: vec![
                ("src/a".into(), None),
                ("src/b".into(), Some("B \"quoted\"".into())),
            ],
            no_server: vec!["src/ui".into()],
            duplicated: vec!["src/shared_stuff".into()],
            keep_prefixes: vec!["use".into()],
            unknown_keys: vec![],
        };
        let toml = to_toml(&imp);
        let s = Settings::from_toml(&toml, Path::new("octatree.toml")).expect(&toml);
        assert!(s.modules.is_empty());
        assert_eq!(s.rules.len(), 4);
        assert_eq!(s.naming.keep_prefixes, ["use"]);
        assert_eq!(s.suffix_rules.len(), 1);
        // Later rules win on equal paths: server-only comes last.
        assert_eq!(s.rules[3].rename.as_deref(), Some("B \"quoted\""));
    }

    #[test]
    fn empty_import_is_still_valid() {
        let toml = to_toml(&Imported::default());
        Settings::from_toml(&toml, Path::new("octatree.toml")).expect(&toml);
    }
}
