use std::{
    cmp::Reverse,
    collections::{HashSet, VecDeque},
    fs,
    io::Cursor,
    path::{Path, PathBuf},
};

use png::{BitDepth, ColorType, Decoder, Transformations};
use tracing::info;

use super::{
    lookup_default_theme,
    rcc::RccArchive,
    svg::decode_svg_with_symbolic_color,
    theme_index::{parse_index_theme, IconDirectory, IconDirectoryType, IconTheme},
    IconImage,
};

#[derive(Debug, Clone)]
pub(crate) struct IconLoader {
    theme_name: String,
    symbolic_color: String,
    search_paths: Vec<PathBuf>,
    pixmaps_paths: Vec<PathBuf>,
    rcc_sources: Vec<RccSource>,
}

#[derive(Debug, Clone)]
struct RccSource {
    archive: RccArchive,
    file_paths: Vec<String>,
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

#[derive(Debug, Clone)]
struct RccCandidate {
    path: String,
    nominal_size: u32,
    extension: String,
}

impl IconLoader {
    /// The (theme_name, symbolic_color) this loader was built with, so an
    /// equivalent loader can be reconstructed on a worker thread for off-loop
    /// icon warming (the loader itself is not shared across threads).
    pub(crate) fn config(&self) -> (&str, &str) {
        (&self.theme_name, &self.symbolic_color)
    }

    pub(crate) fn new_with_symbolic_color(theme_name: &str, symbolic_color: &str) -> Self {
        let search_paths = standard_icon_search_paths();
        let (rcc_sources, total_files) = discover_rcc_sources(&search_paths);
        info!(
            "loaded {} rcc archives with {} total files",
            rcc_sources.len(),
            total_files
        );
        Self {
            theme_name: theme_name.to_string(),
            symbolic_color: symbolic_color.to_string(),
            search_paths,
            pixmaps_paths: vec![
                PathBuf::from("/usr/local/share/pixmaps"),
                PathBuf::from("/usr/share/pixmaps"),
            ],
            rcc_sources,
        }
    }

    #[cfg(test)]
    pub(crate) fn new_for_tests(
        theme_name: &str,
        search_paths: Vec<PathBuf>,
        pixmaps_paths: Vec<PathBuf>,
    ) -> Self {
        let (rcc_sources, _) = discover_rcc_sources(&search_paths);
        Self {
            theme_name: theme_name.to_string(),
            symbolic_color: "#c0caf5".to_string(),
            search_paths,
            pixmaps_paths,
            rcc_sources,
        }
    }

    pub(crate) fn load_icon(&self, name: &str, requested_size: u32) -> Option<IconImage> {
        if name.is_empty() || requested_size == 0 {
            return None;
        }

        self.load_icon_by_name(name, requested_size).or_else(|| {
            icon_aliases(name)
                .iter()
                .find_map(|alias| self.load_icon_by_name(alias, requested_size))
        })
    }

    fn load_icon_by_name(&self, name: &str, requested_size: u32) -> Option<IconImage> {
        for theme_name in self.theme_chain() {
            if let Some(icon) = self.load_icon_from_theme(&theme_name, name, requested_size) {
                return Some(icon);
            }
        }

        self.load_from_pixmaps(name, requested_size)
            .or_else(|| self.load_from_rcc(name, requested_size))
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

    fn load_from_rcc(&self, name: &str, requested_size: u32) -> Option<IconImage> {
        let mut candidates = Vec::new();
        for source in &self.rcc_sources {
            candidates.extend(select_rcc_candidates(
                &source.file_paths,
                name,
                requested_size,
            ));
        }
        let selected = pick_best_rcc_candidate(candidates, requested_size)?;
        for source in &self.rcc_sources {
            let Some(bytes) = source.archive.read_file(&selected.path) else {
                continue;
            };
            return self.decode_icon_bytes(
                &selected.extension,
                &bytes,
                requested_size,
                name.contains("-symbolic"),
            );
        }
        None
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

        let fallback = lookup_default_theme().trim();
        if !fallback.is_empty() && fallback != "hicolor" && !visited.contains(fallback) {
            chain.push(fallback.to_string());
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
        // Symbolic icons live under a `symbolic` directory and/or carry the
        // `-symbolic` filename suffix; those get recoloured to the theme colour.
        let symbolic = path.to_string_lossy().contains("symbolic");
        decode_svg_with_symbolic_color(&data, requested_size, &self.symbolic_color, symbolic)
    }

    fn decode_png_file(&self, path: &Path, requested_size: u32) -> Option<IconImage> {
        let data = fs::read(path).ok()?;
        self.decode_png_bytes(&data, requested_size)
    }

    fn decode_icon_bytes(
        &self,
        extension: &str,
        bytes: &[u8],
        requested_size: u32,
        symbolic: bool,
    ) -> Option<IconImage> {
        match extension {
            "svg" => decode_svg_with_symbolic_color(
                bytes,
                requested_size,
                &self.symbolic_color,
                symbolic,
            ),
            "png" => self.decode_png_bytes(bytes, requested_size),
            _ => None,
        }
    }

    fn decode_png_bytes(&self, bytes: &[u8], requested_size: u32) -> Option<IconImage> {
        let cursor = Cursor::new(bytes);
        let mut decoder = Decoder::new(cursor);
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

fn icon_aliases(name: &str) -> &'static [&'static str] {
    match name {
        "mini.xterm" | "xterm" | "uxterm" => &["utilities-terminal"],
        _ => &[],
    }
}

fn discover_rcc_sources(search_paths: &[PathBuf]) -> (Vec<RccSource>, usize) {
    let mut sources = Vec::new();
    let mut total_files = 0usize;

    for root in search_paths {
        let Ok(theme_dirs) = fs::read_dir(root) else {
            continue;
        };
        for theme_entry in theme_dirs.flatten() {
            let Ok(file_type) = theme_entry.file_type() else {
                continue;
            };
            if !file_type.is_dir() {
                continue;
            }
            let theme_dir = theme_entry.path();
            let Ok(entries) = fs::read_dir(&theme_dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                let is_rcc = path
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .map(|ext| ext.eq_ignore_ascii_case("rcc"))
                    .unwrap_or(false);
                if !is_rcc {
                    continue;
                }

                let Some(archive) = RccArchive::open(&path) else {
                    continue;
                };
                let file_paths = archive.list_files();
                total_files = total_files.saturating_add(file_paths.len());
                sources.push(RccSource {
                    archive,
                    file_paths,
                });
            }
        }
    }

    (sources, total_files)
}

fn select_rcc_candidates(
    file_paths: &[String],
    icon_name: &str,
    requested_size: u32,
) -> Vec<RccCandidate> {
    let mut candidates = Vec::new();
    for file_path in file_paths {
        let Some((base_name, extension)) = split_basename_and_extension(file_path) else {
            continue;
        };
        if base_name != icon_name {
            continue;
        }
        if extension != "png" && extension != "svg" {
            continue;
        }
        let nominal_size = infer_nominal_size(file_path, requested_size);
        candidates.push(RccCandidate {
            path: file_path.clone(),
            nominal_size,
            extension: extension.to_string(),
        });
    }
    candidates
}

fn pick_best_rcc_candidate(
    candidates: Vec<RccCandidate>,
    requested_size: u32,
) -> Option<RccCandidate> {
    if candidates.is_empty() {
        return None;
    }

    let mut preferred: Vec<RccCandidate> = candidates
        .iter()
        .filter(|candidate| candidate.extension == "png")
        .cloned()
        .collect();
    if preferred.is_empty() {
        preferred = candidates;
    }

    let (larger_or_equal, smaller): (Vec<_>, Vec<_>) = preferred
        .into_iter()
        .partition(|candidate| candidate.nominal_size >= requested_size);
    if !larger_or_equal.is_empty() {
        return larger_or_equal
            .into_iter()
            .min_by_key(|candidate| candidate.nominal_size);
    }

    smaller
        .into_iter()
        .max_by_key(|candidate| candidate.nominal_size)
}

fn split_basename_and_extension(path: &str) -> Option<(&str, &str)> {
    let filename = path.rsplit('/').next()?;
    let dot_index = filename.rfind('.')?;
    let base = &filename[..dot_index];
    let extension = &filename[dot_index + 1..];
    if base.is_empty() || extension.is_empty() {
        return None;
    }
    Some((base, extension))
}

fn infer_nominal_size(path: &str, default: u32) -> u32 {
    for segment in path.split('/') {
        let trimmed = segment.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Ok(value) = trimmed.parse::<u32>() {
            return value;
        }
    }
    default
}

include!("loader/filesystem.rs");
include!("loader/decode.rs");

#[cfg(test)]
#[path = "loader_tests.rs"]
mod tests;
