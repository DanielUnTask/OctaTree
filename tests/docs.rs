//! Every ```toml example in the README and docs must be a valid octatree.toml.

use std::fs;
use std::path::{Path, PathBuf};

use octatree::settings::Settings;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn toml_blocks(markdown: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut current: Option<String> = None;
    for line in markdown.lines() {
        match (&mut current, line.trim_end()) {
            (None, "```toml") => current = Some(String::new()),
            (Some(_), "```") => blocks.extend(current.take()),
            (Some(buf), l) => {
                buf.push_str(l);
                buf.push('\n');
            }
            _ => {}
        }
    }
    blocks
}

#[test]
fn documentation_toml_examples_are_valid() {
    let mut files = vec![root().join("README.md")];
    for entry in fs::read_dir(root().join("docs")).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "md") {
            files.push(path);
        }
    }

    let mut checked = 0;
    for file in files {
        let text = fs::read_to_string(&file).unwrap();
        for (i, block) in toml_blocks(&text).iter().enumerate() {
            // `template_file` points at a file that only exists in a real project.
            if block.contains("template_file") {
                continue;
            }
            if let Err(e) = Settings::from_toml(block, Path::new("octatree.toml")) {
                panic!("{} example #{}: {e}\n---\n{block}", file.display(), i + 1);
            }
            checked += 1;
        }
    }
    assert!(checked >= 15, "expected many examples, checked {checked}");
}
