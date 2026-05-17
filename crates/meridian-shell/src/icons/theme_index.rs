use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum IconDirectoryType {
    Fixed,
    Scalable,
    Threshold,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct IconDirectory {
    pub(crate) name: String,
    pub(crate) kind: IconDirectoryType,
    pub(crate) size: u32,
    pub(crate) min_size: u32,
    pub(crate) max_size: u32,
    pub(crate) threshold: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct IconTheme {
    pub(crate) name: String,
    pub(crate) inherits: Vec<String>,
    pub(crate) directories: Vec<IconDirectory>,
}

pub(crate) fn parse_index_theme(input: &str) -> IconTheme {
    let mut section = String::new();
    let mut sections: HashMap<String, HashMap<String, String>> = HashMap::new();

    for raw_line in input.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }

        if line.starts_with('[') && line.ends_with(']') {
            section = line[1..line.len() - 1].trim().to_string();
            sections.entry(section.clone()).or_default();
            continue;
        }

        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if section.is_empty() {
            continue;
        }

        sections
            .entry(section.clone())
            .or_default()
            .insert(key.trim().to_string(), value.trim().to_string());
    }

    let theme_values = sections.get("Icon Theme");
    let theme_name = theme_values
        .and_then(|values| values.get("Name"))
        .cloned()
        .unwrap_or_else(|| "Unnamed".to_string());

    let inherits = split_csv(
        theme_values
            .and_then(|values| values.get("Inherits"))
            .map(|value| value.as_str())
            .unwrap_or_default(),
    );

    let directory_names = split_csv(
        theme_values
            .and_then(|values| values.get("Directories"))
            .map(|value| value.as_str())
            .unwrap_or_default(),
    );

    let mut directories = Vec::new();
    for dir_name in directory_names {
        let Some(values) = sections.get(&dir_name) else {
            continue;
        };

        let kind = match values.get("Type").map(|value| value.trim()) {
            Some("Fixed") => IconDirectoryType::Fixed,
            Some("Scalable") => IconDirectoryType::Scalable,
            Some("Threshold") => IconDirectoryType::Threshold,
            _ => IconDirectoryType::Threshold,
        };
        let size = parse_u32(values.get("Size").map(String::as_str)).unwrap_or(0);
        let min_size = parse_u32(values.get("MinSize").map(String::as_str)).unwrap_or(size);
        let max_size = parse_u32(values.get("MaxSize").map(String::as_str)).unwrap_or(size);
        let threshold = parse_u32(values.get("Threshold").map(String::as_str)).unwrap_or(2);

        directories.push(IconDirectory {
            name: dir_name,
            kind,
            size,
            min_size,
            max_size,
            threshold,
        });
    }

    IconTheme {
        name: theme_name,
        inherits,
        directories,
    }
}

fn split_csv(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|segment| !segment.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn parse_u32(value: Option<&str>) -> Option<u32> {
    value.and_then(|segment| segment.trim().parse::<u32>().ok())
}

#[cfg(test)]
mod tests {
    use super::{parse_index_theme, IconDirectoryType};

    #[test]
    fn parses_minimal_theme_with_inherits_and_directories() {
        let input = r#"
            [Icon Theme]
            Name=Adwaita
            Inherits=AdwaitaLegacy, hicolor
            Directories=22x22/apps, 32x32/apps

            [22x22/apps]
            Size=22
            Type=Fixed

            [32x32/apps]
            Size=32
            Type=Fixed
        "#;

        let theme = parse_index_theme(input);
        assert_eq!(theme.name, "Adwaita");
        assert_eq!(theme.inherits, vec!["AdwaitaLegacy", "hicolor"]);
        assert_eq!(theme.directories.len(), 2);
        assert_eq!(theme.directories[0].name, "22x22/apps");
        assert_eq!(theme.directories[0].kind, IconDirectoryType::Fixed);
        assert_eq!(theme.directories[0].size, 22);
    }

    #[test]
    fn parses_fixed_scalable_and_threshold_directory_types() {
        let input = r#"
            [Icon Theme]
            Name=Theme
            Directories=fixed,scale,thresh

            [fixed]
            Type=Fixed
            Size=16

            [scale]
            Type=Scalable
            Size=64
            MinSize=16
            MaxSize=256

            [thresh]
            Type=Threshold
            Size=48
            Threshold=3
        "#;

        let theme = parse_index_theme(input);
        assert_eq!(theme.directories.len(), 3);
        assert_eq!(theme.directories[0].kind, IconDirectoryType::Fixed);
        assert_eq!(theme.directories[1].kind, IconDirectoryType::Scalable);
        assert_eq!(theme.directories[1].min_size, 16);
        assert_eq!(theme.directories[1].max_size, 256);
        assert_eq!(theme.directories[2].kind, IconDirectoryType::Threshold);
        assert_eq!(theme.directories[2].threshold, 3);
    }

    #[test]
    fn directory_defaults_follow_spec_for_threshold_and_type() {
        let input = r#"
            [Icon Theme]
            Name=Theme
            Directories=apps

            [apps]
            Size=22
        "#;

        let theme = parse_index_theme(input);
        assert_eq!(theme.directories.len(), 1);
        let dir = &theme.directories[0];
        assert_eq!(dir.kind, IconDirectoryType::Threshold);
        assert_eq!(dir.threshold, 2);
        assert_eq!(dir.min_size, 22);
        assert_eq!(dir.max_size, 22);
    }

    #[test]
    fn missing_optional_fields_do_not_fail_parse() {
        let input = r#"
            [Icon Theme]
            Name=Theme
            Directories=apps,actions

            [apps]
            Size=24

            [actions]
            Type=Scalable
            Size=32
        "#;

        let theme = parse_index_theme(input);
        assert_eq!(theme.name, "Theme");
        assert_eq!(theme.inherits.len(), 0);
        assert_eq!(theme.directories.len(), 2);
        assert_eq!(theme.directories[1].min_size, 32);
        assert_eq!(theme.directories[1].max_size, 32);
    }
}
