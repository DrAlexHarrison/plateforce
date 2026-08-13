//! The relation set as one archive, for a surface with no directory to write into.
//!
//! The refusal `write_csv` states holds here by construction: the record is the first entry,
//! so no archive of tables exists without `run.json` beside them. Entries are stored rather
//! than compressed, because the point of this container is that extraction yields the same
//! bytes the disk writer writes, and a reader can verify that with any unzip.

use crate::engine::BatchResult;
use crate::write_csv::EVERY_RELATION;

/// Every entry carries the same fixed timestamp, 1980-01-01 00:00, the format's epoch. A
/// wall-clock stamp would make two archives of one run different files.
const DOS_TIME: u16 = 0;
const DOS_DATE: u16 = 0x0021;

impl BatchResult {
    /// Every relation `write_csv` would put in a directory, as one zip. Entry bytes come
    /// from `relation_text`, so the file a tab saves and the folder the terminal writes are
    /// one rendering, and the same result archives to the same bytes every time.
    pub fn zip_archive(&self) -> Vec<u8> {
        let crc_table = crc32_table();
        let mut archive = Vec::new();
        let mut directory = Vec::new();
        let mut entry_count: u16 = 0;

        for relation in EVERY_RELATION {
            if !self.writes(relation) {
                continue;
            }
            let name = relation.file_name().as_bytes();
            let body = self.relation_text(*relation);
            let bytes = body.as_bytes();
            let size = u32::try_from(bytes.len()).expect("a relation exceeded the zip32 limit");
            let checksum = crc32(&crc_table, bytes);
            let offset =
                u32::try_from(archive.len()).expect("the archive exceeded the zip32 limit");

            archive.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
            archive.extend_from_slice(&20u16.to_le_bytes());
            archive.extend_from_slice(&0u16.to_le_bytes());
            archive.extend_from_slice(&0u16.to_le_bytes());
            archive.extend_from_slice(&DOS_TIME.to_le_bytes());
            archive.extend_from_slice(&DOS_DATE.to_le_bytes());
            archive.extend_from_slice(&checksum.to_le_bytes());
            archive.extend_from_slice(&size.to_le_bytes());
            archive.extend_from_slice(&size.to_le_bytes());
            archive.extend_from_slice(&(name.len() as u16).to_le_bytes());
            archive.extend_from_slice(&0u16.to_le_bytes());
            archive.extend_from_slice(name);
            archive.extend_from_slice(bytes);

            directory.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
            directory.extend_from_slice(&20u16.to_le_bytes());
            directory.extend_from_slice(&20u16.to_le_bytes());
            directory.extend_from_slice(&0u16.to_le_bytes());
            directory.extend_from_slice(&0u16.to_le_bytes());
            directory.extend_from_slice(&DOS_TIME.to_le_bytes());
            directory.extend_from_slice(&DOS_DATE.to_le_bytes());
            directory.extend_from_slice(&checksum.to_le_bytes());
            directory.extend_from_slice(&size.to_le_bytes());
            directory.extend_from_slice(&size.to_le_bytes());
            directory.extend_from_slice(&(name.len() as u16).to_le_bytes());
            directory.extend_from_slice(&0u16.to_le_bytes());
            directory.extend_from_slice(&0u16.to_le_bytes());
            directory.extend_from_slice(&0u16.to_le_bytes());
            directory.extend_from_slice(&0u16.to_le_bytes());
            directory.extend_from_slice(&0u32.to_le_bytes());
            directory.extend_from_slice(&offset.to_le_bytes());
            directory.extend_from_slice(name);
            entry_count += 1;
        }

        let directory_offset =
            u32::try_from(archive.len()).expect("the archive exceeded the zip32 limit");
        let directory_size = u32::try_from(directory.len()).expect("the directory fits zip32");
        archive.extend_from_slice(&directory);
        archive.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
        archive.extend_from_slice(&0u16.to_le_bytes());
        archive.extend_from_slice(&0u16.to_le_bytes());
        archive.extend_from_slice(&entry_count.to_le_bytes());
        archive.extend_from_slice(&entry_count.to_le_bytes());
        archive.extend_from_slice(&directory_size.to_le_bytes());
        archive.extend_from_slice(&directory_offset.to_le_bytes());
        archive.extend_from_slice(&0u16.to_le_bytes());
        archive
    }
}

fn crc32_table() -> [u32; 256] {
    let mut table = [0u32; 256];
    let mut index = 0usize;
    while index < 256 {
        let mut value = index as u32;
        let mut bit = 0;
        while bit < 8 {
            value = if value & 1 != 0 {
                (value >> 1) ^ 0xEDB8_8320
            } else {
                value >> 1
            };
            bit += 1;
        }
        table[index] = value;
        index += 1;
    }
    table
}

fn crc32(table: &[u32; 256], bytes: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &byte in bytes {
        crc = (crc >> 8) ^ table[((crc ^ byte as u32) & 0xFF) as usize];
    }
    !crc
}

/// The stored entries read back out of an archive, checksums held to their bytes. The
/// read-back for the check that what a tab downloads is what the disk writer writes.
pub fn read_archive(archive: &[u8]) -> Result<Vec<(String, Vec<u8>)>, String> {
    let table = crc32_table();
    let mut entries = Vec::new();
    let mut at = 0usize;
    let read_u16 = |bytes: &[u8], from: usize| u16::from_le_bytes([bytes[from], bytes[from + 1]]);
    let read_u32 = |bytes: &[u8], from: usize| {
        u32::from_le_bytes([
            bytes[from],
            bytes[from + 1],
            bytes[from + 2],
            bytes[from + 3],
        ])
    };
    while at + 4 <= archive.len() && read_u32(archive, at) == 0x0403_4b50 {
        let checksum = read_u32(archive, at + 14);
        let size = read_u32(archive, at + 18) as usize;
        let name_length = read_u16(archive, at + 26) as usize;
        let extra_length = read_u16(archive, at + 28) as usize;
        let name_at = at + 30;
        let body_at = name_at + name_length + extra_length;
        if body_at + size > archive.len() {
            return Err("an entry runs past the end of the archive".to_string());
        }
        let name = String::from_utf8(archive[name_at..name_at + name_length].to_vec())
            .map_err(|error| format!("an entry name is not text: {error}"))?;
        let body = archive[body_at..body_at + size].to_vec();
        if crc32(&table, &body) != checksum {
            return Err(format!(
                "{name} carries a checksum its bytes do not produce"
            ));
        }
        entries.push((name, body));
        at = body_at + size;
    }
    if entries.is_empty() {
        return Err("no stored entry opens this archive".to_string());
    }
    Ok(entries)
}
