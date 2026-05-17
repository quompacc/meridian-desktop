use std::{
    cmp::Reverse,
    collections::{HashSet, VecDeque},
    fs,
    path::{Path, PathBuf},
};

use png::{BitDepth, ColorType, Decoder, Transformations};

use super::{
    svg::decode_svg,
    theme_index::{parse_index_theme, IconDirectory, IconDirectoryType, IconTheme},
    IconImage,
};

#[derive(Debug, Clone)]
pub(crate) struct IconLoader {
    theme_name: String,
    search_paths: Vec<PathBuf>,
    pixmaps_paths: Vec<PathBuf>,
}

#[derive(Debug)]
struct ThemeLocation {
    root: PathBuf,
    theme: IconTheme,
}

#[derive(Debug)]
struct IconCandidate {
    path: PathBuf,
    nominal_size: u32,
    exact: bool,
}

impl IconLoader {
    pub(crate) fn new(theme_name: &str) -> Self {
        Self {
            theme_name: theme_name.to_string(),
            search_paths: standard_icon_search_paths(),
            pixmaps_paths: vec![PathBuf::from("/usr/share/pixmaps")],
        }
    }

    #[cfg(test)]
    pub(crate) fn new_for_tests(
        theme_name: &str,
        search_paths: Vec<PathBuf>,
        pixmaps_paths: Vec<PathBuf>,
    ) -> Self {
        Self {
            theme_name: theme_name.to_string(),
            search_paths,
            pixmaps_paths,
        }
    }

    pub(crate) fn load_icon(&self, name: &str, requested_size: u32) -> Option<IconImage> {
        if name.is_empty() || requested_size == 0 {
            return None;
        }

        for theme_name in self.theme_chain() {
            if let Some(icon) = self.load_icon_from_theme(&theme_name, name, requested_size) {
                return Some(icon);
            }
        }

        self.load_from_pixmaps(name, requested_size)
    }

    pub(crate) fn load_icon_from_absolute_path(
        &self,
        path: &str,
        requested_size: u32,
    ) -> Option<IconImage> {
        if requested_size == 0 || !path.starts_with('/') {
            return None;
        }
        self.decode_icon_path(Path::new(path), requested_size)
    }

    fn load_icon_from_theme(
        &self,
        theme_name: &str,
        name: &str,
        requested_size: u32,
    ) -> Option<IconImage> {
        let ThemeLocation { root, theme } = self.load_theme(theme_name)?;
        let mut candidates = Vec::new();
        for directory in &theme.directories {
            let png_path = root
                .join(theme_name)
                .join(&directory.name)
                .join(format!("{name}.png"));
            let icon_path = if png_path.is_file() {
                png_path
            } else {
                let svg_path = root
                    .join(theme_name)
                    .join(&directory.name)
                    .join(format!("{name}.svg"));
                if !svg_path.is_file() {
                    continue;
                }
                svg_path
            };

            candidates.push(IconCandidate {
                path: icon_path,
                nominal_size: directory.size,
                exact: directory_matches_size(directory, requested_size),
            });
        }

        let selected = pick_best_candidate(candidates, requested_size)?;
        self.decode_icon_path(&selected.path, requested_size)
    }

    fn load_theme(&self, theme_name: &str) -> Option<ThemeLocation> {
        for root in &self.search_paths {
            let index_path = root.join(theme_name).join("index.theme");
            let Ok(raw) = fs::read_to_string(&index_path) else {
                continue;
            };
            let theme = parse_index_theme(&raw);
            return Some(ThemeLocation {
                root: root.clone(),
                theme,
            });
        }
        None
    }

    fn theme_chain(&self) -> Vec<String> {
        let mut chain = Vec::new();
        let mut visited: HashSet<String> = HashSet::new();
        let mut queue = VecDeque::new();
        queue.push_back(self.theme_name.clone());

        while let Some(theme_name) = queue.pop_front() {
            let trimmed = theme_name.trim();
            if trimmed.is_empty() {
                continue;
            }
            let normalized = trimmed.to_string();
            if !visited.insert(normalized.clone()) {
                continue;
            }

            if normalized != "hicolor" {
                chain.push(normalized.clone());
            }

            if let Some(theme) = self.load_theme(&normalized).map(|location| location.theme) {
                for parent in theme.inherits {
                    if !parent.trim().is_empty() {
                        queue.push_back(parent);
                    }
                }
            }
        }

        chain.push("hicolor".to_string());
        chain
    }

    fn load_from_pixmaps(&self, name: &str, requested_size: u32) -> Option<IconImage> {
        for root in &self.pixmaps_paths {
            let path = root.join(format!("{name}.png"));
            if path.is_file() {
                return self.decode_png_file(&path, requested_size);
            }
        }
        None
    }

    fn decode_icon_path(&self, path: &Path, requested_size: u32) -> Option<IconImage> {
        let extension = path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_ascii_lowercase());
        match extension.as_deref() {
            Some("svg") => self.decode_svg_file(path, requested_size),
            Some("png") | None => self.decode_png_file(path, requested_size),
            _ => None,
        }
    }

    fn decode_svg_file(&self, path: &Path, requested_size: u32) -> Option<IconImage> {
        let data = fs::read(path).ok()?;
        decode_svg(&data, requested_size)
    }

    fn decode_png_file(&self, path: &Path, requested_size: u32) -> Option<IconImage> {
        let file = fs::File::open(path).ok()?;
        let mut decoder = Decoder::new(file);
        decoder.set_transformations(Transformations::EXPAND | Transformations::STRIP_16);
        let mut reader = decoder.read_info().ok()?;
        if reader.info().color_type == ColorType::Indexed {
            return None;
        }

        let mut buffer = vec![0; reader.output_buffer_size()];
        let frame = reader.next_frame(&mut buffer).ok()?;
        let bytes = &buffer[..frame.buffer_size()];
        let rgba = decode_to_rgba8(
            bytes,
            frame.color_type,
            frame.bit_depth,
            frame.width,
            frame.height,
        )?;
        let bgra = rgba_to_bgra(&rgba);

        if frame.width == requested_size && frame.height == requested_size {
            return Some(IconImage {
                width: frame.width,
                height: frame.height,
                bgra,
            });
        }

        let resized = resize_bilinear_bgra(
            &bgra,
            frame.width,
            frame.height,
            requested_size,
            requested_size,
        );
        Some(IconImage {
            width: requested_size,
            height: requested_size,
            bgra: resized,
        })
    }
}

pub(crate) fn standard_icon_search_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    let home = std::env::var_os("HOME").map(PathBuf::from);

    let data_home = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| home.as_ref().map(|h| h.join(".local/share")));
    if let Some(base) = data_home {
        paths.push(base.join("icons"));
    }

    if let Some(home_dir) = home {
        paths.push(home_dir.join(".icons"));
    }

    let xdg_data_dirs = std::env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/share".to_string());
    for base in xdg_data_dirs
        .split(':')
        .filter(|segment| !segment.trim().is_empty())
    {
        paths.push(PathBuf::from(base).join("icons"));
    }

    dedupe_paths(paths)
}

fn dedupe_paths(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut seen = HashSet::new();
    let mut deduped = Vec::new();
    for path in paths {
        if seen.insert(path.clone()) {
            deduped.push(path);
        }
    }
    deduped
}

fn directory_matches_size(directory: &IconDirectory, size: u32) -> bool {
    match directory.kind {
        IconDirectoryType::Fixed => directory.size == size,
        IconDirectoryType::Scalable => directory.min_size <= size && size <= directory.max_size,
        IconDirectoryType::Threshold => {
            let min = directory.size.saturating_sub(directory.threshold);
            let max = directory.size.saturating_add(directory.threshold);
            min <= size && size <= max
        }
    }
}

fn pick_best_candidate(
    candidates: Vec<IconCandidate>,
    requested_size: u32,
) -> Option<IconCandidate> {
    if candidates.is_empty() {
        return None;
    }

    let (exact_candidates, non_exact): (Vec<_>, Vec<_>) = candidates
        .into_iter()
        .partition(|candidate| candidate.exact);
    if !exact_candidates.is_empty() {
        return choose_nearest(exact_candidates, requested_size);
    }

    let (larger_or_equal, smaller): (Vec<_>, Vec<_>) = non_exact
        .into_iter()
        .partition(|candidate| candidate.nominal_size >= requested_size);
    if !larger_or_equal.is_empty() {
        return choose_smallest_size(larger_or_equal);
    }

    choose_largest_size(smaller)
}

fn choose_nearest(candidates: Vec<IconCandidate>, requested_size: u32) -> Option<IconCandidate> {
    candidates.into_iter().min_by_key(|candidate| {
        (
            distance(candidate.nominal_size, requested_size),
            Reverse(candidate.nominal_size),
        )
    })
}

fn choose_smallest_size(candidates: Vec<IconCandidate>) -> Option<IconCandidate> {
    candidates
        .into_iter()
        .min_by_key(|candidate| candidate.nominal_size)
}

fn choose_largest_size(candidates: Vec<IconCandidate>) -> Option<IconCandidate> {
    candidates
        .into_iter()
        .max_by_key(|candidate| candidate.nominal_size)
}

fn distance(a: u32, b: u32) -> u32 {
    a.abs_diff(b)
}

fn decode_to_rgba8(
    src: &[u8],
    color_type: ColorType,
    bit_depth: BitDepth,
    width: u32,
    height: u32,
) -> Option<Vec<u8>> {
    if bit_depth != BitDepth::Eight {
        return None;
    }

    let pixels = width as usize * height as usize;
    let mut rgba = vec![0u8; pixels * 4];

    match color_type {
        ColorType::Rgba => {
            if src.len() != pixels * 4 {
                return None;
            }
            rgba.copy_from_slice(src);
        }
        ColorType::Rgb => {
            if src.len() != pixels * 3 {
                return None;
            }
            for (index, chunk) in src.chunks_exact(3).enumerate() {
                let out = index * 4;
                rgba[out] = chunk[0];
                rgba[out + 1] = chunk[1];
                rgba[out + 2] = chunk[2];
                rgba[out + 3] = 255;
            }
        }
        ColorType::Grayscale => {
            if src.len() != pixels {
                return None;
            }
            for (index, gray) in src.iter().enumerate() {
                let out = index * 4;
                rgba[out] = *gray;
                rgba[out + 1] = *gray;
                rgba[out + 2] = *gray;
                rgba[out + 3] = 255;
            }
        }
        ColorType::GrayscaleAlpha => {
            if src.len() != pixels * 2 {
                return None;
            }
            for (index, chunk) in src.chunks_exact(2).enumerate() {
                let out = index * 4;
                rgba[out] = chunk[0];
                rgba[out + 1] = chunk[0];
                rgba[out + 2] = chunk[0];
                rgba[out + 3] = chunk[1];
            }
        }
        ColorType::Indexed => {
            return None;
        }
    }

    Some(rgba)
}

fn rgba_to_bgra(rgba: &[u8]) -> Vec<u8> {
    let mut bgra = Vec::with_capacity(rgba.len());
    for chunk in rgba.chunks_exact(4) {
        bgra.push(chunk[2]);
        bgra.push(chunk[1]);
        bgra.push(chunk[0]);
        bgra.push(chunk[3]);
    }
    bgra
}

pub(crate) fn resize_bilinear_bgra(
    src: &[u8],
    src_w: u32,
    src_h: u32,
    dst_w: u32,
    dst_h: u32,
) -> Vec<u8> {
    if src_w == dst_w && src_h == dst_h {
        return src.to_vec();
    }

    let mut dst = vec![0u8; (dst_w * dst_h * 4) as usize];
    let src_wf = src_w as f32;
    let src_hf = src_h as f32;
    let dst_wf = dst_w as f32;
    let dst_hf = dst_h as f32;

    for y in 0..dst_h {
        let fy = ((y as f32 + 0.5) * src_hf / dst_hf - 0.5).clamp(0.0, (src_h - 1) as f32);
        let y0 = fy.floor() as u32;
        let y1 = (y0 + 1).min(src_h - 1);
        let wy = fy - y0 as f32;

        for x in 0..dst_w {
            let fx = ((x as f32 + 0.5) * src_wf / dst_wf - 0.5).clamp(0.0, (src_w - 1) as f32);
            let x0 = fx.floor() as u32;
            let x1 = (x0 + 1).min(src_w - 1);
            let wx = fx - x0 as f32;

            let idx00 = ((y0 * src_w + x0) * 4) as usize;
            let idx10 = ((y0 * src_w + x1) * 4) as usize;
            let idx01 = ((y1 * src_w + x0) * 4) as usize;
            let idx11 = ((y1 * src_w + x1) * 4) as usize;
            let out = ((y * dst_w + x) * 4) as usize;

            for channel in 0..4 {
                let p00 = src[idx00 + channel] as f32;
                let p10 = src[idx10 + channel] as f32;
                let p01 = src[idx01 + channel] as f32;
                let p11 = src[idx11 + channel] as f32;

                let top = p00 + (p10 - p00) * wx;
                let bottom = p01 + (p11 - p01) * wx;
                let value = top + (bottom - top) * wy;
                dst[out + channel] = value.round().clamp(0.0, 255.0) as u8;
            }
        }
    }

    dst
}

#[cfg(test)]
mod tests {
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
        let mut body =
            format!("[Icon Theme]\nName=Theme\nInherits={inherits}\nDirectories={dirs}\n\n");

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
}
