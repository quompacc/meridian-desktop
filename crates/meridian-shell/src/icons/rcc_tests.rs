// Only the Linux-gated integration test below uses `Path`; gate the import
// to match so non-Linux builds (e.g. FreeBSD) don't warn on an unused import.
#[cfg(target_os = "linux")]
use std::path::Path;

use super::{parse_header, RccArchive, FLAG_DIRECTORY, FLAG_ZLIB, FLAG_ZSTD};

#[derive(Clone)]
struct TestNode {
    name: &'static str,
    kind: TestNodeKind,
}

#[derive(Clone)]
enum TestNodeKind {
    Directory {
        child_count: u32,
        first_child: u32,
        flags: u16,
    },
    File {
        flags: u16,
        payload: Vec<u8>,
    },
}

fn build_rcc(nodes: &[TestNode]) -> Vec<u8> {
    let mut names_section = Vec::new();
    let mut name_offsets = Vec::with_capacity(nodes.len());
    for node in nodes {
        let offset = names_section.len() as u32;
        name_offsets.push(offset);

        let utf16: Vec<u16> = node.name.encode_utf16().collect();
        names_section.extend_from_slice(&(utf16.len() as u16).to_be_bytes());
        names_section.extend_from_slice(&0u32.to_be_bytes());
        for unit in utf16 {
            names_section.extend_from_slice(&unit.to_be_bytes());
        }
    }

    let mut data_section = Vec::new();
    let mut file_offsets = vec![0u32; nodes.len()];
    for (index, node) in nodes.iter().enumerate() {
        if let TestNodeKind::File { payload, .. } = &node.kind {
            file_offsets[index] = data_section.len() as u32;
            data_section.extend_from_slice(&(payload.len() as u32).to_be_bytes());
            data_section.extend_from_slice(payload);
        }
    }

    let mut tree_section = Vec::new();
    for (index, node) in nodes.iter().enumerate() {
        tree_section.extend_from_slice(&name_offsets[index].to_be_bytes());
        match &node.kind {
            TestNodeKind::Directory {
                child_count,
                first_child,
                flags,
            } => {
                tree_section.extend_from_slice(&flags.to_be_bytes());
                tree_section.extend_from_slice(&child_count.to_be_bytes());
                tree_section.extend_from_slice(&first_child.to_be_bytes());
                tree_section.extend_from_slice(&0u64.to_be_bytes());
            }
            TestNodeKind::File { flags, .. } => {
                tree_section.extend_from_slice(&flags.to_be_bytes());
                tree_section.extend_from_slice(&0u16.to_be_bytes());
                tree_section.extend_from_slice(&0u16.to_be_bytes());
                tree_section.extend_from_slice(&file_offsets[index].to_be_bytes());
                tree_section.extend_from_slice(&0u64.to_be_bytes());
            }
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

fn minimal_root_with_file(file_name: &'static str, flags: u16, payload: Vec<u8>) -> Vec<u8> {
    build_rcc(&[
        TestNode {
            name: "",
            kind: TestNodeKind::Directory {
                child_count: 1,
                first_child: 1,
                flags: FLAG_DIRECTORY,
            },
        },
        TestNode {
            name: file_name,
            kind: TestNodeKind::File { flags, payload },
        },
    ])
}

// Real-World-Layout: header -> data -> names -> tree (tree als letzte Sektion).
fn build_rcc_data_names_tree(nodes: &[TestNode]) -> Vec<u8> {
    let mut names_section = Vec::new();
    let mut name_offsets = Vec::with_capacity(nodes.len());
    for node in nodes {
        let offset = names_section.len() as u32;
        name_offsets.push(offset);

        let utf16: Vec<u16> = node.name.encode_utf16().collect();
        names_section.extend_from_slice(&(utf16.len() as u16).to_be_bytes());
        names_section.extend_from_slice(&0u32.to_be_bytes());
        for unit in utf16 {
            names_section.extend_from_slice(&unit.to_be_bytes());
        }
    }

    let mut data_section = Vec::new();
    let mut file_offsets = vec![0u32; nodes.len()];
    for (index, node) in nodes.iter().enumerate() {
        if let TestNodeKind::File { payload, .. } = &node.kind {
            file_offsets[index] = data_section.len() as u32;
            data_section.extend_from_slice(&(payload.len() as u32).to_be_bytes());
            data_section.extend_from_slice(payload);
        }
    }

    let mut tree_section = Vec::new();
    for (index, node) in nodes.iter().enumerate() {
        tree_section.extend_from_slice(&name_offsets[index].to_be_bytes());
        match &node.kind {
            TestNodeKind::Directory {
                child_count,
                first_child,
                flags,
            } => {
                tree_section.extend_from_slice(&flags.to_be_bytes());
                tree_section.extend_from_slice(&child_count.to_be_bytes());
                tree_section.extend_from_slice(&first_child.to_be_bytes());
                tree_section.extend_from_slice(&0u64.to_be_bytes());
            }
            TestNodeKind::File { flags, .. } => {
                tree_section.extend_from_slice(&flags.to_be_bytes());
                tree_section.extend_from_slice(&0u16.to_be_bytes());
                tree_section.extend_from_slice(&0u16.to_be_bytes());
                tree_section.extend_from_slice(&file_offsets[index].to_be_bytes());
                tree_section.extend_from_slice(&0u64.to_be_bytes());
            }
        }
    }

    let data_offset = 24u32;
    let names_offset = data_offset + data_section.len() as u32;
    let tree_offset = names_offset + names_section.len() as u32;

    let mut out = Vec::new();
    out.extend_from_slice(&0x7172_6573u32.to_be_bytes());
    out.extend_from_slice(&3u32.to_be_bytes());
    out.extend_from_slice(&tree_offset.to_be_bytes());
    out.extend_from_slice(&data_offset.to_be_bytes());
    out.extend_from_slice(&names_offset.to_be_bytes());
    out.extend_from_slice(&0u32.to_be_bytes());
    out.extend_from_slice(&data_section);
    out.extend_from_slice(&names_section);
    out.extend_from_slice(&tree_section);
    out
}

#[test]
fn parse_header_returns_none_for_wrong_magic() {
    let mut raw = vec![0u8; 24];
    raw[0..4].copy_from_slice(b"junk");
    raw[4..8].copy_from_slice(&3u32.to_be_bytes());
    raw[8..12].copy_from_slice(&24u32.to_be_bytes());
    raw[12..16].copy_from_slice(&24u32.to_be_bytes());
    raw[16..20].copy_from_slice(&24u32.to_be_bytes());
    assert!(RccArchive::from_test_bytes(raw).is_none());
}

#[test]
fn parse_header_returns_none_for_wrong_version() {
    let mut raw = vec![0u8; 24];
    raw[0..4].copy_from_slice(&0x7172_6573u32.to_be_bytes());
    raw[4..8].copy_from_slice(&99u32.to_be_bytes());
    raw[8..12].copy_from_slice(&24u32.to_be_bytes());
    raw[12..16].copy_from_slice(&24u32.to_be_bytes());
    raw[16..20].copy_from_slice(&24u32.to_be_bytes());
    assert!(RccArchive::from_test_bytes(raw).is_none());
}

#[test]
fn parse_header_accepts_v3_with_expected_offsets() {
    let raw = minimal_root_with_file("foo", 0, b"bar".to_vec());
    let header = parse_header(&raw).expect("v3 header parses");
    assert_eq!(header.data_offset, 24);
    assert!(header.tree_offset > header.data_offset);
    assert!(header.names_offset > header.tree_offset);
}

#[test]
fn tree_walk_finds_root_directory_listing() {
    let archive = RccArchive::from_test_bytes(minimal_root_with_file("foo", 0, b"bar".to_vec()))
        .expect("archive parses");
    assert_eq!(archive.list_files(), vec!["foo".to_string()]);
}

#[test]
fn tree_walk_navigates_subdirectories() {
    let archive = RccArchive::from_test_bytes(build_rcc(&[
        TestNode {
            name: "",
            kind: TestNodeKind::Directory {
                child_count: 1,
                first_child: 1,
                flags: FLAG_DIRECTORY,
            },
        },
        TestNode {
            name: "dir",
            kind: TestNodeKind::Directory {
                child_count: 1,
                first_child: 2,
                flags: FLAG_DIRECTORY,
            },
        },
        TestNode {
            name: "foo",
            kind: TestNodeKind::File {
                flags: 0,
                payload: b"payload".to_vec(),
            },
        },
    ]))
    .expect("archive parses");

    assert_eq!(archive.list_files(), vec!["dir/foo".to_string()]);
    assert_eq!(
        archive.read_file("dir/foo").expect("payload"),
        b"payload".to_vec()
    );
}

#[test]
fn tree_walk_works_when_tree_is_last_section() {
    let raw = build_rcc_data_names_tree(&[
        TestNode {
            name: "",
            kind: TestNodeKind::Directory {
                child_count: 1,
                first_child: 1,
                flags: FLAG_DIRECTORY,
            },
        },
        TestNode {
            name: "foo",
            kind: TestNodeKind::File {
                flags: 0,
                payload: b"bar".to_vec(),
            },
        },
    ]);
    let archive = RccArchive::from_test_bytes(raw).expect("real-world layout parses");
    assert_eq!(archive.list_files(), vec!["foo".to_string()]);
    assert_eq!(archive.read_file("foo").expect("payload"), b"bar".to_vec());
}

#[test]
fn read_file_decompresses_zstd_payload() {
    let compressed = vec![
        0x28, 0xb5, 0x2f, 0xfd, 0x04, 0x58, 0x89, 0x00, 0x00, 0x6d, 0x65, 0x72, 0x69, 0x64, 0x69,
        0x61, 0x6e, 0x2d, 0x72, 0x63, 0x63, 0x2d, 0x7a, 0x73, 0x74, 0x64, 0x34, 0xee, 0x85, 0x68,
    ];
    let archive =
        RccArchive::from_test_bytes(minimal_root_with_file("zstd.bin", FLAG_ZSTD, compressed))
            .expect("archive parses");

    assert_eq!(
        archive.read_file("zstd.bin").expect("decompressed"),
        b"meridian-rcc-zstd".to_vec()
    );
}

#[test]
fn read_file_rejects_zlib_compressed_files() {
    let archive = RccArchive::from_test_bytes(minimal_root_with_file(
        "zlib.bin",
        FLAG_ZLIB,
        b"not-supported".to_vec(),
    ))
    .expect("archive parses");
    assert!(archive.read_file("zlib.bin").is_none());
}

#[test]
fn lookup_nonexistent_path_returns_none() {
    let archive = RccArchive::from_test_bytes(minimal_root_with_file("foo", 0, b"x".to_vec()))
        .expect("archive parses");
    assert!(archive.read_file("missing").is_none());
    assert!(archive.read_file("foo/bar").is_none());
}

#[test]
#[cfg(target_os = "linux")]
fn integration_real_breeze_rcc_can_be_opened_and_query_known_icon() {
    let path = Path::new("/usr/share/icons/breeze/breeze-icons.rcc");
    if !path.exists() {
        eprintln!("skipping; /usr/share/icons/breeze/breeze-icons.rcc is absent");
        return;
    }

    let archive =
        RccArchive::open(path).expect("real breeze-icons.rcc must parse after Phase 7b-2 hotfix");

    let files = archive.list_files();
    assert!(
        files.len() > 1000,
        "unexpectedly low file count: {}",
        files.len()
    );
    let probe = files
        .iter()
        .find(|entry| entry.ends_with(".svg"))
        .or_else(|| {
            files
                .iter()
                .find(|entry| entry.ends_with(".png") || entry.ends_with(".xpm"))
        });
    let Some(probe_path) = probe else {
        eprintln!("skipping; no icon-like files found");
        return;
    };

    let bytes = archive.read_file(probe_path).expect("resource bytes");
    assert!(!bytes.is_empty());
}
