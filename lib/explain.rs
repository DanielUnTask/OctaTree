//! `octatree explain <path>`: shows how a path is routed and which rules decide it.

use std::fmt::Write as _;

use crate::error::Result;
use crate::scan::DirReader;
use crate::settings::{Mask, Settings, Source};

fn describe_mask(s: &Settings, mask: Mask) -> String {
    let names = s.mask_names(mask);
    if names.is_empty() {
        "nowhere (excluded)".into()
    } else {
        format!("[{}]", names.join(", "))
    }
}

fn describe_source(s: &Settings, src: Source) -> String {
    match src {
        Source::Rule(i) => format!("rule {}", s.rules[i].label),
        Source::Suffix(i) => format!(
            "suffix rule #{i} ({})",
            s.suffix_rules[i].suffixes.join("/")
        ),
        Source::Inherited => "inherited from an ancestor directory rule".into(),
        Source::Default => "default_to".into(),
    }
}

/// Explains the routing of `input` (project-relative path) using the real
/// directory contents exposed by `reader`.
pub fn explain<R: DirReader>(s: &Settings, input: &str, reader: &mut R) -> Result<String> {
    let mut out = String::new();
    let path = input.replace('\\', "/");
    let path = path
        .trim_start_matches("./")
        .trim_end_matches('/')
        .to_string();
    let _ = writeln!(out, "{path}");

    let Some(rest) = path
        .strip_prefix(s.source_dir.as_str())
        .and_then(|r| r.strip_prefix('/'))
    else {
        let _ = writeln!(out, "  not under source_dir `{}`", s.source_dir);
        return Ok(out);
    };
    let segs: Vec<&str> = rest.split('/').collect();
    if !s.modules.iter().any(|m| m == segs[0]) {
        let _ = writeln!(
            out,
            "  `{}` is not in `modules` ({}): never scanned",
            segs[0],
            s.modules.join(", ")
        );
        return Ok(out);
    }

    let mut buf = Vec::new();
    let mut inherited: Option<Mask> = None;
    let mut cur = s.source_dir.clone();
    for (i, seg) in segs.iter().enumerate() {
        cur.push('/');
        cur.push_str(seg);
        let last = i + 1 == segs.len();
        let _ = writeln!(out, "  {cur}");

        if s.is_ignored(&cur) {
            let _ = writeln!(out, "    ignored by an `ignore` pattern: skipped");
            return Ok(out);
        }
        let d = s.direct(&cur, &mut buf);
        for &r in &d.rules {
            let rule = &s.rules[r];
            let mut effects = Vec::new();
            if let Some(m) = rule.to {
                effects.push(format!("to {}", describe_mask(s, m)));
            }
            if let Some(n) = &rule.rename {
                effects.push(format!("rename `{n}`"));
            }
            if let Some(class) = &rule.class_name {
                effects.push(format!("class `{class}`"));
            }
            if rule.set.is_some() {
                effects.push("set properties".into());
            }
            let _ = writeln!(out, "    rule {}: {}", rule.label, effects.join(", "));
        }

        let entries = reader.read_dir(&cur)?;
        let is_dir = entries.is_some();
        if let Some(entries) = &entries
            && let Some(init) = entries.iter().find(|e| s.is_init_file(&e.name))
        {
            let _ = writeln!(
                out,
                "    folder contains `{}`: mounted as ONE node",
                init.name
            );
            if !last {
                let _ = writeln!(out, "    files inside are not routed individually");
                return Ok(out);
            }
            let (mask, src) = s.decide(seg, &d, inherited);
            let _ = writeln!(out, "    name: {}", s.node_name(seg, &d));
            let _ = writeln!(
                out,
                "    => {} via {}",
                describe_mask(s, mask),
                describe_source(s, src)
            );
            return Ok(out);
        }

        if is_dir {
            if let Some((m, _)) = d.to {
                inherited = Some(m);
                let _ = writeln!(
                    out,
                    "    directory target for descendants: {}",
                    describe_mask(s, m)
                );
            }
            if last {
                let _ = writeln!(out, "    directory: its files are routed individually");
            }
            continue;
        }

        // A file (or a path that does not exist).
        if !last {
            let _ = writeln!(out, "    does not exist on disk");
            return Ok(out);
        }
        let Some(stem) = s.strip_extension(seg) else {
            let _ = writeln!(
                out,
                "    not a source file (extensions: {}): ignored",
                s.extensions.join(", ")
            );
            return Ok(out);
        };
        let (mask, src) = s.decide(stem, &d, inherited);
        let _ = writeln!(out, "    name: {}", s.node_name(stem, &d));
        let _ = writeln!(
            out,
            "    => {} via {}",
            describe_mask(s, mask),
            describe_source(s, src)
        );
    }
    Ok(out)
}
