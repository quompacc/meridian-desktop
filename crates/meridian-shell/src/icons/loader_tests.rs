use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use png::{BitDepth, ColorType, Encoder};

use super::IconLoader;

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(label: &str) -> Self {
        let mut path = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        path.push(format!(
            "meridian-shell-{label}-{}-{nanos}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("create temp dir");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn write_theme_index(path: &Path, inherits: &str, directories: &[&str]) {
    let dirs = directories.join(",");
    let mut body = format!("[Icon Theme]\nName=Theme\nInherits={inherits}\nDirectories={dirs}\n\n");

    for directory in directories {
        let size: u32 = directory
            .split('x')
            .next()
            .unwrap_or("0")
            .parse()
            .unwrap_or(0);
        body.push_str(&format!("[{directory}]\nType=Fixed\nSize={size}\n\n"));
    }

    fs::write(path, body).expect("write index.theme");
}

fn write_png_rgba(path: &Path, width: u32, height: u32, rgba: [u8; 4]) {
    let file = fs::File::create(path).expect("create png");
    let mut encoder = Encoder::new(file, width, height);
    encoder.set_color(ColorType::Rgba);
    encoder.set_depth(BitDepth::Eight);
    let mut writer = encoder.write_header().expect("write header");
    let mut data = Vec::with_capacity((width * height * 4) as usize);
    for _ in 0..(width * height) {
        data.extend_from_slice(&rgba);
    }
    writer.write_image_data(&data).expect("write data");
}

fn write_png_indexed(path: &Path, width: u32, height: u32) {
    let file = fs::File::create(path).expect("create png");
    let mut encoder = Encoder::new(file, width, height);
    encoder.set_color(ColorType::Indexed);
    encoder.set_depth(BitDepth::Eight);
    encoder.set_palette(vec![255, 0, 0, 0, 255, 0]);
    let mut writer = encoder.write_header().expect("write header");
    let mut data = Vec::with_capacity((width * height) as usize);
    for i in 0..(width * height) {
        data.push((i % 2) as u8);
    }
    writer.write_image_data(&data).expect("write data");
}

fn write_svg_solid_rect(path: &Path, color: &str) {
    let body = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' width='24' height='24'><rect width='24' height='24' fill='{color}'/></svg>"
    );
    fs::write(path, body).expect("write svg");
}

fn make_theme_root(base: &Path, theme: &str, dirs: &[&str], inherits: &str) {
    let theme_root = base.join(theme);
    fs::create_dir_all(&theme_root).expect("create theme root");
    write_theme_index(&theme_root.join("index.theme"), inherits, dirs);
    for dir in dirs {
        fs::create_dir_all(theme_root.join(dir)).expect("create icon dir");
    }
}

fn build_chain_rcc(internal_path: &str, payload: &[u8], file_flags: u16) -> Vec<u8> {
    const DIRECTORY_FLAG: u16 = 0x02;
    let segments: Vec<&str> = internal_path
        .split('/')
        .filter(|segment| !segment.trim().is_empty())
        .collect();
    assert!(
        !segments.is_empty(),
        "path must contain at least one segment"
    );

    let node_count = segments.len() + 1;
    let mut names_section = Vec::new();
    let mut name_offsets = Vec::with_capacity(node_count);

    name_offsets.push(0u32);
    names_section.extend_from_slice(&0u16.to_be_bytes());
    names_section.extend_from_slice(&0u32.to_be_bytes());

    for segment in &segments {
        let offset = names_section.len() as u32;
        name_offsets.push(offset);
        let utf16: Vec<u16> = segment.encode_utf16().collect();
        names_section.extend_from_slice(&(utf16.len() as u16).to_be_bytes());
        names_section.extend_from_slice(&0u32.to_be_bytes());
        for unit in utf16 {
            names_section.extend_from_slice(&unit.to_be_bytes());
        }
    }

    let mut data_section = Vec::new();
    data_section.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    data_section.extend_from_slice(payload);

    let mut tree_section = Vec::new();
    for (index, name_offset) in name_offsets.iter().enumerate().take(node_count) {
        tree_section.extend_from_slice(&name_offset.to_be_bytes());
        if index < node_count - 1 {
            tree_section.extend_from_slice(&DIRECTORY_FLAG.to_be_bytes());
            tree_section.extend_from_slice(&1u32.to_be_bytes());
            tree_section.extend_from_slice(&((index + 1) as u32).to_be_bytes());
            tree_section.extend_from_slice(&0u64.to_be_bytes());
        } else {
            tree_section.extend_from_slice(&file_flags.to_be_bytes());
            tree_section.extend_from_slice(&0u16.to_be_bytes());
            tree_section.extend_from_slice(&0u16.to_be_bytes());
            tree_section.extend_from_slice(&0u32.to_be_bytes());
            tree_section.extend_from_slice(&0u64.to_be_bytes());
        }
    }

    let data_offset = 24u32;
    let tree_offset = data_offset + data_section.len() as u32;
    let names_offset = tree_offset + tree_section.len() as u32;

    let mut out = Vec::new();
    out.extend_from_slice(&0x7172_6573u32.to_be_bytes());
    out.extend_from_slice(&3u32.to_be_bytes());
    out.extend_from_slice(&tree_offset.to_be_bytes());
    out.extend_from_slice(&data_offset.to_be_bytes());
    out.extend_from_slice(&names_offset.to_be_bytes());
    out.extend_from_slice(&0u32.to_be_bytes());
    out.extend_from_slice(&data_section);
    out.extend_from_slice(&tree_section);
    out.extend_from_slice(&names_section);
    out
}

fn write_rcc_file(path: &Path, internal_path: &str, payload: &[u8], file_flags: u16) {
    fs::write(path, build_chain_rcc(internal_path, payload, file_flags)).expect("write rcc");
}

#[test]
fn lookup_finds_icon_in_theme_directory() {
    let temp = TempDir::new("lookup-theme");
    let icons_root = temp.path().join("icons");
    fs::create_dir_all(&icons_root).expect("create icons root");

    make_theme_root(&icons_root, "Adwaita", &["22x22/apps"], "");
    write_png_rgba(
        &icons_root
            .join("Adwaita")
            .join("22x22/apps")
            .join("utilities-terminal.png"),
        22,
        22,
        [255, 0, 0, 255],
    );

    let loader = IconLoader::new_for_tests("Adwaita", vec![icons_root], vec![]);
    let icon = loader.load_icon("utilities-terminal", 22).expect("icon");
    assert_eq!(icon.width, 22);
    assert_eq!(icon.height, 22);
    assert_eq!(&icon.bgra[0..4], &[0, 0, 255, 255]);
}

#[test]
fn lookup_falls_back_to_default_theme_before_hicolor() {
    let temp = TempDir::new("default-theme-fallback");
    let icons_root = temp.path().join("icons");
    fs::create_dir_all(&icons_root).expect("create icons root");

    make_theme_root(&icons_root, "Papirus", &["22x22/apps"], "");
    make_theme_root(&icons_root, "breeze", &["22x22/actions"], "");
    write_png_rgba(
        &icons_root
            .join("breeze")
            .join("22x22/actions")
            .join("system-shutdown.png"),
        22,
        22,
        [1, 2, 3, 255],
    );

    let loader = IconLoader::new_for_tests("Papirus", vec![icons_root], vec![]);
    let icon = loader
        .load_icon("system-shutdown", 22)
        .expect("fallback icon");
    assert_eq!(icon.width, 22);
    assert_eq!(icon.height, 22);
    assert_eq!(&icon.bgra[0..4], &[3, 2, 1, 255]);
}

#[test]
fn lookup_uses_terminal_alias_for_mini_xterm() {
    let temp = TempDir::new("terminal-alias");
    let icons_root = temp.path().join("icons");
    fs::create_dir_all(&icons_root).expect("create icons root");

    make_theme_root(&icons_root, "Papirus", &["22x22/apps"], "");
    write_png_rgba(
        &icons_root
            .join("Papirus")
            .join("22x22/apps")
            .join("utilities-terminal.png"),
        22,
        22,
        [4, 5, 6, 255],
    );

    let loader = IconLoader::new_for_tests("Papirus", vec![icons_root], vec![]);
    let icon = loader.load_icon("mini.xterm", 22).expect("alias icon");
    assert_eq!(&icon.bgra[0..4], &[6, 5, 4, 255]);
}

#[test]
fn absolute_path_loader_loads_png() {
    let temp = TempDir::new("absolute-path");
    let png_path = temp.path().join("standalone.png");
    write_png_rgba(&png_path, 24, 24, [1, 2, 3, 255]);

    let loader = IconLoader::new_for_tests("Adwaita", vec![], vec![]);
    let icon = loader
        .load_icon_from_absolute_path(png_path.to_str().expect("utf8 path"), 24)
        .expect("icon from absolute path");
    assert_eq!(icon.width, 24);
    assert_eq!(icon.height, 24);
    assert_eq!(&icon.bgra[0..4], &[3, 2, 1, 255]);
}

#[test]
fn svg_in_theme_directory_is_loaded() {
    let temp = TempDir::new("svg-theme");
    let icons_root = temp.path().join("icons");
    fs::create_dir_all(&icons_root).expect("create icons root");

    make_theme_root(&icons_root, "Adwaita", &["22x22/apps"], "");
    write_svg_solid_rect(
        &icons_root
            .join("Adwaita")
            .join("22x22/apps")
            .join("only-svg.svg"),
        "#00ff00",
    );

    let loader = IconLoader::new_for_tests("Adwaita", vec![icons_root], vec![]);
    let icon = loader.load_icon("only-svg", 22).expect("svg icon");
    assert_eq!(icon.width, 22);
    assert_eq!(icon.height, 22);
}

#[test]
fn prefers_png_over_svg_in_same_directory() {
    let temp = TempDir::new("prefer-png");
    let icons_root = temp.path().join("icons");
    fs::create_dir_all(&icons_root).expect("create icons root");

    make_theme_root(&icons_root, "Adwaita", &["22x22/apps"], "");
    let base = icons_root.join("Adwaita").join("22x22/apps");
    write_png_rgba(&base.join("dual.png"), 22, 22, [255, 0, 0, 255]);
    write_svg_solid_rect(&base.join("dual.svg"), "#00ff00");

    let loader = IconLoader::new_for_tests("Adwaita", vec![icons_root], vec![]);
    let icon = loader.load_icon("dual", 22).expect("dual icon");
    assert_eq!(&icon.bgra[0..4], &[0, 0, 255, 255]);
}

#[test]
fn falls_back_to_svg_when_no_png() {
    let temp = TempDir::new("fallback-svg");
    let icons_root = temp.path().join("icons");
    fs::create_dir_all(&icons_root).expect("create icons root");

    make_theme_root(&icons_root, "Adwaita", &["22x22/apps"], "");
    write_svg_solid_rect(
        &icons_root
            .join("Adwaita")
            .join("22x22/apps")
            .join("svg-only.svg"),
        "#0000ff",
    );

    let loader = IconLoader::new_for_tests("Adwaita", vec![icons_root], vec![]);
    let icon = loader.load_icon("svg-only", 22).expect("svg fallback icon");
    assert_eq!(icon.width, 22);
    assert_eq!(icon.height, 22);
}

#[test]
fn absolute_path_loader_loads_svg() {
    let temp = TempDir::new("absolute-svg");
    let svg_path = temp.path().join("standalone.svg");
    write_svg_solid_rect(&svg_path, "#ff00ff");

    let loader = IconLoader::new_for_tests("Adwaita", vec![], vec![]);
    let icon = loader
        .load_icon_from_absolute_path(svg_path.to_str().expect("utf8 path"), 24)
        .expect("svg icon from absolute path");
    assert_eq!(icon.width, 24);
    assert_eq!(icon.height, 24);
}

#[test]
fn size_selection_prefers_closest_larger_before_upscaling_smaller() {
    let temp = TempDir::new("size-select");
    let icons_root = temp.path().join("icons");
    fs::create_dir_all(&icons_root).expect("create icons root");

    make_theme_root(&icons_root, "Adwaita", &["16x16/apps", "32x32/apps"], "");
    write_png_rgba(
        &icons_root
            .join("Adwaita")
            .join("16x16/apps")
            .join("firefox.png"),
        16,
        16,
        [0, 255, 0, 255],
    );
    write_png_rgba(
        &icons_root
            .join("Adwaita")
            .join("32x32/apps")
            .join("firefox.png"),
        32,
        32,
        [255, 0, 0, 255],
    );

    let loader = IconLoader::new_for_tests("Adwaita", vec![icons_root], vec![]);
    let icon = loader.load_icon("firefox", 22).expect("icon");
    assert_eq!(icon.width, 22);
    assert_eq!(icon.height, 22);
    assert_eq!(&icon.bgra[0..4], &[0, 0, 255, 255]);
}

#[test]
fn inheritance_chain_finds_parent_theme_icon() {
    let temp = TempDir::new("inherits");
    let icons_root = temp.path().join("icons");
    fs::create_dir_all(&icons_root).expect("create icons root");

    make_theme_root(&icons_root, "Adwaita", &["22x22/apps"], "AdwaitaLegacy");
    make_theme_root(&icons_root, "AdwaitaLegacy", &["22x22/apps"], "");
    write_png_rgba(
        &icons_root
            .join("AdwaitaLegacy")
            .join("22x22/apps")
            .join("utilities-terminal.png"),
        22,
        22,
        [7, 8, 9, 255],
    );

    let loader = IconLoader::new_for_tests("Adwaita", vec![icons_root], vec![]);
    let icon = loader.load_icon("utilities-terminal", 22).expect("icon");
    assert_eq!(&icon.bgra[0..4], &[9, 8, 7, 255]);
}

#[test]
fn pixmaps_fallback_is_used_when_theme_lookup_misses() {
    let temp = TempDir::new("pixmaps");
    let icons_root = temp.path().join("icons");
    let pixmaps_root = temp.path().join("pixmaps");
    fs::create_dir_all(&icons_root).expect("create icons root");
    fs::create_dir_all(&pixmaps_root).expect("create pixmaps root");

    make_theme_root(&icons_root, "Adwaita", &["22x22/apps"], "");
    write_png_rgba(
        &pixmaps_root.join("my-flat-icon.png"),
        22,
        22,
        [32, 64, 128, 255],
    );

    let loader = IconLoader::new_for_tests("Adwaita", vec![icons_root], vec![pixmaps_root]);
    let icon = loader.load_icon("my-flat-icon", 22).expect("icon");
    assert_eq!(&icon.bgra[0..4], &[128, 64, 32, 255]);
}

#[test]
fn downscale_produces_requested_dimensions() {
    let temp = TempDir::new("downscale");
    let icons_root = temp.path().join("icons");
    fs::create_dir_all(&icons_root).expect("create icons root");

    make_theme_root(&icons_root, "Adwaita", &["32x32/apps"], "");
    write_png_rgba(
        &icons_root
            .join("Adwaita")
            .join("32x32/apps")
            .join("firefox.png"),
        32,
        32,
        [100, 110, 120, 255],
    );

    let loader = IconLoader::new_for_tests("Adwaita", vec![icons_root], vec![]);
    let icon = loader.load_icon("firefox", 22).expect("icon");
    assert_eq!(icon.width, 22);
    assert_eq!(icon.height, 22);
}

#[test]
fn indexed_palette_png_is_skipped_without_panic() {
    let temp = TempDir::new("indexed");
    let icons_root = temp.path().join("icons");
    fs::create_dir_all(&icons_root).expect("create icons root");

    make_theme_root(&icons_root, "Adwaita", &["22x22/apps"], "");
    write_png_indexed(
        &icons_root
            .join("Adwaita")
            .join("22x22/apps")
            .join("palette.png"),
        22,
        22,
    );

    let loader = IconLoader::new_for_tests("Adwaita", vec![icons_root], vec![]);
    assert!(loader.load_icon("palette", 22).is_none());
}

#[test]
fn rcc_archives_are_discovered_in_theme_directory() {
    let temp = TempDir::new("rcc-discovery");
    let icons_root = temp.path().join("icons");
    let theme_root = icons_root.join("breeze");
    fs::create_dir_all(&theme_root).expect("create theme root");
    write_rcc_file(
        &theme_root.join("breeze-icons.rcc"),
        "icons/breeze/actions/22/run.svg",
        b"<svg xmlns='http://www.w3.org/2000/svg' width='22' height='22'></svg>",
        0x00,
    );

    let loader = IconLoader::new_for_tests("Adwaita", vec![icons_root], vec![]);
    assert_eq!(loader.rcc_archive_count(), 1);
    assert!(loader.rcc_total_file_count() >= 1);
}

#[test]
fn lookup_falls_back_to_rcc_after_fs_exhausted() {
    let temp = TempDir::new("rcc-fallback");
    let icons_root = temp.path().join("icons");
    fs::create_dir_all(&icons_root).expect("create icons root");
    make_theme_root(&icons_root, "Adwaita", &["22x22/apps"], "");

    let breeze_theme_root = icons_root.join("breeze");
    fs::create_dir_all(&breeze_theme_root).expect("create breeze theme root");
    write_rcc_file(
        &breeze_theme_root.join("breeze-icons.rcc"),
        "icons/breeze/apps/22/rcc-only.svg",
        b"<svg xmlns='http://www.w3.org/2000/svg' width='22' height='22'><rect width='22' height='22' fill='#ff0000'/></svg>",
        0x00,
    );

    let loader = IconLoader::new_for_tests("Adwaita", vec![icons_root], vec![]);
    let icon = loader.load_icon("rcc-only", 22).expect("icon from rcc");
    assert_eq!(icon.width, 22);
    assert_eq!(icon.height, 22);
}

#[test]
fn lookup_prefers_fs_over_rcc_when_both_present() {
    let temp = TempDir::new("rcc-priority");
    let icons_root = temp.path().join("icons");
    fs::create_dir_all(&icons_root).expect("create icons root");
    make_theme_root(&icons_root, "Adwaita", &["22x22/apps"], "");
    write_png_rgba(
        &icons_root
            .join("Adwaita")
            .join("22x22/apps")
            .join("shared.png"),
        22,
        22,
        [1, 2, 3, 255],
    );

    let breeze_theme_root = icons_root.join("breeze");
    fs::create_dir_all(&breeze_theme_root).expect("create breeze theme root");
    write_rcc_file(
        &breeze_theme_root.join("breeze-icons.rcc"),
        "icons/breeze/apps/22/shared.svg",
        b"<svg xmlns='http://www.w3.org/2000/svg' width='22' height='22'><rect width='22' height='22' fill='#00ff00'/></svg>",
        0x00,
    );

    let loader = IconLoader::new_for_tests("Adwaita", vec![icons_root], vec![]);
    let icon = loader.load_icon("shared", 22).expect("icon");
    assert_eq!(&icon.bgra[0..4], &[3, 2, 1, 255]);
}
