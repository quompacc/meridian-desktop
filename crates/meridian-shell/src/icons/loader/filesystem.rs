impl IconLoader {
    #[cfg(test)]
    pub(crate) fn rcc_archive_count(&self) -> usize {
        self.rcc_sources.len()
    }

    #[cfg(test)]
    pub(crate) fn rcc_total_file_count(&self) -> usize {
        self.rcc_sources
            .iter()
            .map(|source| source.file_paths.len())
            .sum()
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

    // Default must include /usr/local/share: that is where FreeBSD pkg installs
    // every icon theme (breeze, Adwaita, …). A bare "/usr/share" fallback leaves
    // the search path empty on FreeBSD, so no theme resolves and the launcher —
    // which hides apps whose icon cannot be found — comes up empty.
    let xdg_data_dirs = std::env::var("XDG_DATA_DIRS")
        .unwrap_or_else(|_| "/usr/local/share:/usr/share".to_string());
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
