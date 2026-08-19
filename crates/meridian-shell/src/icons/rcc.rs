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
#[path = "rcc_tests.rs"]
mod tests;
