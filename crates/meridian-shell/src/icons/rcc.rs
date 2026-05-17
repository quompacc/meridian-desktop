use std::{char::decode_utf16, fs, io::Read, path::Path};

use ruzstd::StreamingDecoder;

const RCC_HEADER_LEN: usize = 24;
const RCC_NODE_LEN: usize = 22;
const MAGIC_QRES: u32 = 0x7172_6573;
const SUPPORTED_VERSION: u32 = 3;
const FLAG_ZLIB: u16 = 0x01;
const FLAG_DIRECTORY: u16 = 0x02;
const FLAG_ZSTD: u16 = 0x04;

#[derive(Debug, Clone)]
pub(crate) struct RccArchive {
    raw: Vec<u8>,
    tree_offset: usize,
    data_offset: usize,
    names_offset: usize,
    node_count: usize,
}

#[derive(Debug, Clone, Copy)]
struct Header {
    tree_offset: usize,
    data_offset: usize,
    names_offset: usize,
}

#[derive(Debug, Clone, Copy)]
enum RccNode {
    Directory {
        name_offset: u32,
        child_count: u32,
        first_child: u32,
    },
    File {
        name_offset: u32,
        flags: u16,
        data_payload_offset: u32,
    },
}

impl RccArchive {
    pub(crate) fn open(path: &Path) -> Option<Self> {
        let raw = fs::read(path).ok()?;
        Self::from_bytes(raw)
    }

    #[cfg(test)]
    fn from_test_bytes(raw: Vec<u8>) -> Option<Self> {
        Self::from_bytes(raw)
    }

    fn from_bytes(raw: Vec<u8>) -> Option<Self> {
        let header = parse_header(&raw)?;
        // Tree-Section endet entweder am Anfang der nächsten Sektion (wenn eine danach
        // kommt) oder am Dateiende.
        let next_section_start = [header.data_offset, header.names_offset]
            .into_iter()
            .filter(|&start| start > header.tree_offset)
            .min()
            .unwrap_or(raw.len());
        let tree_len = next_section_start.checked_sub(header.tree_offset)?;
        if tree_len == 0 || tree_len % RCC_NODE_LEN != 0 {
            return None;
        }
        let node_count = tree_len / RCC_NODE_LEN;
        let archive = Self {
            raw,
            tree_offset: header.tree_offset,
            data_offset: header.data_offset,
            names_offset: header.names_offset,
            node_count,
        };
        match archive.parse_node(0)? {
            RccNode::Directory { .. } => Some(archive),
            RccNode::File { .. } => None,
        }
    }

    pub(crate) fn read_file(&self, path: &str) -> Option<Vec<u8>> {
        let mut current = 0usize;
        for segment in path.split('/').filter(|part| !part.is_empty()) {
            current = self.find_child_by_name(current, segment)?;
        }

        let RccNode::File {
            flags,
            data_payload_offset,
            ..
        } = self.parse_node(current)?
        else {
            return None;
        };

        let payload = self.raw_file_payload(data_payload_offset)?;
        decode_payload(flags, payload)
    }

    pub(crate) fn list_files(&self) -> Vec<String> {
        let mut files = Vec::new();
        self.collect_files(0, String::new(), &mut files);
        files
    }

    fn collect_files(&self, index: usize, prefix: String, files: &mut Vec<String>) {
        match self.parse_node(index) {
            Some(RccNode::Directory {
                child_count,
                first_child,
                ..
            }) => {
                let start = first_child as usize;
                let end = match start.checked_add(child_count as usize) {
                    Some(end) if end <= self.node_count => end,
                    _ => return,
                };
                for child_index in start..end {
                    let Some(child) = self.parse_node(child_index) else {
                        continue;
                    };
                    let Some(name) = self.node_name(child) else {
                        continue;
                    };
                    let next_prefix = if prefix.is_empty() {
                        name
                    } else {
                        format!("{prefix}/{name}")
                    };
                    self.collect_files(child_index, next_prefix, files);
                }
            }
            Some(RccNode::File { .. }) if !prefix.is_empty() => files.push(prefix),
            Some(RccNode::File { .. }) => {}
            None => {}
        }
    }

    fn find_child_by_name(&self, directory_index: usize, expected_name: &str) -> Option<usize> {
        let RccNode::Directory {
            child_count,
            first_child,
            ..
        } = self.parse_node(directory_index)?
        else {
            return None;
        };

        let start = first_child as usize;
        let end = start.checked_add(child_count as usize)?;
        if end > self.node_count {
            return None;
        }

        for child_index in start..end {
            let child = self.parse_node(child_index)?;
            if self.node_name(child).as_deref() == Some(expected_name) {
                return Some(child_index);
            }
        }
        None
    }

    fn raw_file_payload(&self, data_payload_offset: u32) -> Option<&[u8]> {
        let data_offset = self.data_offset.checked_add(data_payload_offset as usize)?;
        let payload_len = read_u32_be(&self.raw, data_offset)? as usize;
        let payload_start = data_offset.checked_add(4)?;
        let payload_end = payload_start.checked_add(payload_len)?;
        self.raw.get(payload_start..payload_end)
    }

    fn parse_node(&self, index: usize) -> Option<RccNode> {
        if index >= self.node_count {
            return None;
        }
        let base = self
            .tree_offset
            .checked_add(index.checked_mul(RCC_NODE_LEN)?)?;
        let name_offset = read_u32_be(&self.raw, base)?;
        let flags = read_u16_be(&self.raw, base + 4)?;

        if flags & FLAG_DIRECTORY != 0 {
            let child_count = read_u32_be(&self.raw, base + 6)?;
            let first_child = read_u32_be(&self.raw, base + 10)?;
            Some(RccNode::Directory {
                name_offset,
                child_count,
                first_child,
            })
        } else {
            let data_payload_offset = read_u32_be(&self.raw, base + 10)?;
            Some(RccNode::File {
                name_offset,
                flags,
                data_payload_offset,
            })
        }
    }

    fn node_name(&self, node: RccNode) -> Option<String> {
        match node {
            RccNode::Directory { name_offset, .. } | RccNode::File { name_offset, .. } => {
                self.read_name(name_offset)
            }
        }
    }

    fn read_name(&self, name_offset: u32) -> Option<String> {
        let name_entry = self.names_offset.checked_add(name_offset as usize)?;
        let units_len = read_u16_be(&self.raw, name_entry)? as usize;
        let data_start = name_entry.checked_add(6)?;
        let data_end = data_start.checked_add(units_len.checked_mul(2)?)?;
        let bytes = self.raw.get(data_start..data_end)?;

        let mut units = Vec::with_capacity(units_len);
        for chunk in bytes.chunks_exact(2) {
            units.push(u16::from_be_bytes([chunk[0], chunk[1]]));
        }

        let mut decoded = String::with_capacity(units_len);
        for unit in decode_utf16(units) {
            decoded.push(unit.ok()?);
        }
        Some(decoded)
    }
}

fn decode_payload(flags: u16, payload: &[u8]) -> Option<Vec<u8>> {
    if flags & FLAG_ZLIB != 0 {
        return None;
    }
    if flags & FLAG_ZSTD != 0 {
        let mut decoder = StreamingDecoder::new(payload).ok()?;
        let mut decoded = Vec::new();
        decoder.read_to_end(&mut decoded).ok()?;
        return Some(decoded);
    }
    Some(payload.to_vec())
}

fn parse_header(raw: &[u8]) -> Option<Header> {
    if raw.len() < RCC_HEADER_LEN {
        return None;
    }
    if read_u32_be(raw, 0)? != MAGIC_QRES {
        return None;
    }
    if read_u32_be(raw, 4)? != SUPPORTED_VERSION {
        return None;
    }

    let tree_offset = read_u32_be(raw, 8)? as usize;
    let data_offset = read_u32_be(raw, 12)? as usize;
    let names_offset = read_u32_be(raw, 16)? as usize;

    if tree_offset > raw.len() || data_offset > raw.len() || names_offset > raw.len() {
        return None;
    }

    Some(Header {
        tree_offset,
        data_offset,
        names_offset,
    })
}

fn read_u16_be(raw: &[u8], offset: usize) -> Option<u16> {
    let end = offset.checked_add(2)?;
    let bytes = raw.get(offset..end)?;
    Some(u16::from_be_bytes([bytes[0], bytes[1]]))
}

fn read_u32_be(raw: &[u8], offset: usize) -> Option<u32> {
    let end = offset.checked_add(4)?;
    let bytes = raw.get(offset..end)?;
    Some(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

#[cfg(test)]
mod tests {
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
        let archive =
            RccArchive::from_test_bytes(minimal_root_with_file("foo", 0, b"bar".to_vec()))
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
            0x28, 0xb5, 0x2f, 0xfd, 0x04, 0x58, 0x89, 0x00, 0x00, 0x6d, 0x65, 0x72, 0x69, 0x64,
            0x69, 0x61, 0x6e, 0x2d, 0x72, 0x63, 0x63, 0x2d, 0x7a, 0x73, 0x74, 0x64, 0x34, 0xee,
            0x85, 0x68,
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

        let archive = RccArchive::open(path)
            .expect("real breeze-icons.rcc must parse after Phase 7b-2 hotfix");

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
}
