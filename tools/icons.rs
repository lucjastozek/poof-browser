//! Build-time icon generation from the `@mingcute/svg` npm package. Called from `build.rs`.
//!
//! There is no icon list to maintain: every complete string literal in the UI sources that is a
//! MingCute icon name gets embedded. `name` is the outline style, `name-fill` the filled one.
//! The sidebar also gets all other outline icons as a gzipped pack (for the space icon picker).

use base64::Engine;
use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    io::Write,
    path::{Path, PathBuf},
};

const PACKAGE: &str = "node_modules/@mingcute/svg";
/// Holds Lucide-to-MingCute aliases; their targets stay in the pack until a space needs one.
const SCAN_SKIP: &str = "icon-aliases.js";
/// The sidebar is one `NavigateToString` page that must stay under 2 MB, scripts and styles included.
const SIDEBAR_ICONS_MAX: usize = 1_500_000;

type Icons = BTreeMap<String, String>;

pub fn generate() {
    let root = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    for path in [
        "build.rs",
        "tools",
        "ui",
        "src/browser/js/devtools-chrome.js",
        PACKAGE,
    ] {
        println!("cargo:rerun-if-changed={path}");
    }

    let package = root.join(PACKAGE);
    assert!(
        package.is_dir(),
        "{PACKAGE} is missing: run `npm install` in the project root"
    );
    let outline = load(&package.join("core-regular"), "");
    let mut all = load(&package.join("core-filled"), "-fill");
    all.extend(outline.clone());

    // Sidebar: referenced icons as plain text, every other outline icon packed.
    let mut sources = Vec::new();
    collect(&root.join("ui/scripts"), &mut sources);
    // Evaluated inside the sidebar, so it draws from the same set.
    sources.push(root.join("src/browser/js/devtools-chrome.js"));
    let embedded = referenced(&sources, &all);
    let rest: Icons = outline
        .into_iter()
        .filter(|(name, _)| !embedded.contains_key(name))
        .collect();
    let mut gzip = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::best());
    gzip.write_all(json(&rest).as_bytes()).unwrap();
    let packed = base64::engine::general_purpose::STANDARD.encode(gzip.finish().unwrap());
    let sidebar = format!(
        "/* MingCute (Apache-2.0, https://www.mingcute.com). */\n\
         const ICONS = {};\nconst ICONS_PACKED = \"{packed}\";\n",
        json(&embedded)
    );
    assert!(
        sidebar.len() <= SIDEBAR_ICONS_MAX,
        "sidebar icons are {} bytes, over the {SIDEBAR_ICONS_MAX} limit",
        sidebar.len()
    );
    write(&out, "icons.js", &sidebar);

    // PDF viewer: only what it references.
    let mut sources = Vec::new();
    collect(&root.join("ui/pdf/js"), &mut sources);
    let pdf = format!("const ICONS = {};\n", json(&referenced(&sources, &all)));
    write(&out, "pdf-icons.js", &pdf);
}

/// Reads one style folder. Only the markup inside `<svg>` is kept; the UI supplies the wrapper.
fn load(dir: &Path, suffix: &str) -> Icons {
    let mut icons = Icons::new();
    for entry in fs::read_dir(dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display())) {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|ext| ext != "svg") {
            continue;
        }
        let name = path.file_stem().unwrap().to_string_lossy();
        let svg = fs::read_to_string(&path).unwrap();
        let start = svg.find("<svg").expect("not an SVG");
        let open = start + svg[start..].find('>').expect("not an SVG") + 1;
        let close = svg.rfind("</svg>").expect("not an SVG");
        icons.insert(
            format!("{name}{suffix}"),
            svg[open..close].trim().to_string(),
        );
    }
    icons
}

fn collect(dir: &Path, files: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display())) {
        let path = entry.unwrap().path();
        if path.is_dir() {
            collect(&path, files);
        } else if path.extension().is_some_and(|ext| ext == "js")
            && path.file_name().is_some_and(|name| name != SCAN_SKIP)
        {
            files.push(path);
        }
    }
}

/// Icons whose names appear as string literals in `files`. A name passed straight to an icon
/// helper must exist, so a typo fails the build instead of leaving a blank button.
fn referenced(files: &[PathBuf], all: &Icons) -> Icons {
    const CALLS: &[&str] = &["ic(\"", "ib(\"", "iconBtn(\"", "actionBtn(\"", "icon: \""];
    let mut names = BTreeSet::new();
    for file in files {
        let text = fs::read_to_string(file).unwrap();
        for call in CALLS {
            for (at, _) in text.match_indices(call) {
                let name = text[at + call.len()..].split('"').next().unwrap_or("");
                assert!(
                    name.is_empty() || all.contains_key(name),
                    "{}: no MingCute icon named `{name}`",
                    file.display()
                );
            }
        }
        let bytes = text.as_bytes();
        for (start, &quote) in bytes.iter().enumerate() {
            if !matches!(quote, b'"' | b'\'' | b'`') {
                continue;
            }
            let tail = &bytes[start + 1..];
            let len = tail
                .iter()
                .take_while(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || **b == b'-')
                .count();
            if len > 0 && tail.get(len) == Some(&quote) {
                names.insert(text[start + 1..start + 1 + len].to_string());
            }
        }
    }
    all.iter()
        .filter(|(name, _)| names.contains(name.as_str()))
        .map(|(name, svg)| (name.clone(), svg.clone()))
        .collect()
}

fn json(icons: &Icons) -> String {
    serde_json::to_string(icons).unwrap()
}

fn write(dir: &Path, name: &str, text: &str) {
    fs::write(dir.join(name), text).unwrap();
}
