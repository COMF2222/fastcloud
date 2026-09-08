//! Just enough zip to open a Winamp skin.
//!
//! A `.wsz` is a plain zip of BMP sprite sheets and two text files. Skins are
//! tiny and hand-made, so this reads the central directory and inflates the
//! members it is asked for — `stored` and `deflate`, which is everything any
//! skin in the wild uses. No crate is pulled in for it: `flate2` is already
//! compiled for reqwest's gzip support, so this is only the container.
//!
//! Names are matched case-insensitively and by their last path segment,
//! because skins disagree about `MAIN.BMP` vs `main.bmp` and some wrap
//! everything in a folder.

use anyhow::{Context as _, Result, bail};
use std::collections::HashMap;
use std::io::Read as _;

/// End of central directory record, as the zip spec names it.
const EOCD_SIGNATURE: [u8; 4] = [0x50, 0x4b, 0x05, 0x06];
const CENTRAL_SIGNATURE: [u8; 4] = [0x50, 0x4b, 0x01, 0x02];
const LOCAL_SIGNATURE: [u8; 4] = [0x50, 0x4b, 0x03, 0x04];
/// A skin is a few hundred KB; this caps a hostile or corrupt member.
const MAX_MEMBER_BYTES: u64 = 32 * 1024 * 1024;

/// One member of the archive, as found in the central directory.
#[derive(Debug, Clone)]
struct Member {
    /// Lowercased last path segment, e.g. `main.bmp`.
    name: String,
    method: u16,
    compressed_size: u64,
    uncompressed_size: u64,
    /// Offset of the local file header.
    offset: u64,
}

/// An opened archive: the directory is parsed, members are inflated on demand.
pub struct Archive {
    data: Vec<u8>,
    members: HashMap<String, Member>,
}

impl Archive {
    /// Read the central directory. Fails when this is not a zip at all.
    pub fn open(data: Vec<u8>) -> Result<Self> {
        let eocd = find_eocd(&data).context("not a zip archive (no end-of-directory record)")?;
        let count = u16::from_le_bytes([data[eocd + 10], data[eocd + 11]]) as usize;
        let mut offset = u32::from_le_bytes([
            data[eocd + 16],
            data[eocd + 17],
            data[eocd + 18],
            data[eocd + 19],
        ]) as usize;
        let mut members = HashMap::with_capacity(count);
        for _ in 0..count {
            if offset + 46 > data.len() || data[offset..offset + 4] != CENTRAL_SIGNATURE {
                break;
            }
            let read16 = |at: usize| u16::from_le_bytes([data[offset + at], data[offset + at + 1]]);
            let read32 = |at: usize| {
                u32::from_le_bytes([
                    data[offset + at],
                    data[offset + at + 1],
                    data[offset + at + 2],
                    data[offset + at + 3],
                ])
            };
            let method = read16(10);
            let compressed_size = u64::from(read32(20));
            let uncompressed_size = u64::from(read32(24));
            let name_len = read16(28) as usize;
            let extra_len = read16(30) as usize;
            let comment_len = read16(32) as usize;
            let local_offset = u64::from(read32(42));
            let name_at = offset + 46;
            if name_at + name_len > data.len() {
                break;
            }
            let raw = String::from_utf8_lossy(&data[name_at..name_at + name_len]).into_owned();
            // Directories end in a slash and carry no data.
            if !raw.ends_with('/') {
                let key = leaf_name(&raw);
                members.insert(
                    key.clone(),
                    Member {
                        name: key,
                        method,
                        compressed_size,
                        uncompressed_size,
                        offset: local_offset,
                    },
                );
            }
            offset = name_at + name_len + extra_len + comment_len;
        }
        if members.is_empty() {
            bail!("the archive holds no files");
        }
        Ok(Self { data, members })
    }

    /// Every member's name, lowercased leaf.
    #[allow(dead_code)]
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.members.keys().map(String::as_str)
    }

    #[allow(dead_code)]
    pub fn contains(&self, name: &str) -> bool {
        self.members.contains_key(&leaf_name(name))
    }

    /// Inflate one member. `None` when the archive has no such file.
    pub fn read(&self, name: &str) -> Option<Result<Vec<u8>>> {
        let member = self.members.get(&leaf_name(name))?;
        Some(self.inflate(member))
    }

    fn inflate(&self, member: &Member) -> Result<Vec<u8>> {
        if member.uncompressed_size > MAX_MEMBER_BYTES {
            bail!(
                "{} is {} bytes, refusing to unpack",
                member.name,
                member.uncompressed_size
            );
        }
        let header = member.offset as usize;
        if header + 30 > self.data.len() || self.data[header..header + 4] != LOCAL_SIGNATURE {
            bail!("{}: broken local header", member.name);
        }
        // The local header repeats the name and extra field with their own
        // lengths; the data starts after them.
        let name_len =
            u16::from_le_bytes([self.data[header + 26], self.data[header + 27]]) as usize;
        let extra_len =
            u16::from_le_bytes([self.data[header + 28], self.data[header + 29]]) as usize;
        let start = header + 30 + name_len + extra_len;
        let end = start + member.compressed_size as usize;
        if end > self.data.len() {
            bail!("{}: truncated archive", member.name);
        }
        let raw = &self.data[start..end];
        match member.method {
            0 => Ok(raw.to_vec()),
            8 => {
                let mut out = Vec::with_capacity(member.uncompressed_size as usize);
                flate2::read::DeflateDecoder::new(raw)
                    .take(MAX_MEMBER_BYTES)
                    .read_to_end(&mut out)
                    .with_context(|| format!("{}: inflate failed", member.name))?;
                Ok(out)
            }
            other => bail!("{}: unsupported compression method {other}", member.name),
        }
    }
}

/// `Skin/Folder/MAIN.BMP` → `main.bmp`.
fn leaf_name(path: &str) -> String {
    path.rsplit(['/', '\\'])
        .next()
        .unwrap_or(path)
        .to_lowercase()
}

/// Scan backwards for the end-of-directory record (it is last, but a comment
/// may follow it).
fn find_eocd(data: &[u8]) -> Option<usize> {
    if data.len() < 22 {
        return None;
    }
    let start = data.len().saturating_sub(66_000);
    (start..=data.len() - 22)
        .rev()
        .find(|&i| data[i..i + 4] == EOCD_SIGNATURE)
}

/// Builders the skin tests share. Not compiled into the app.
#[cfg(test)]
pub mod tests_support {
    use super::{CENTRAL_SIGNATURE, EOCD_SIGNATURE, LOCAL_SIGNATURE};

    /// A one-member zip built by hand: `stored`, so no compressor is needed.
    pub fn stored_zip(name: &str, body: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        let name_bytes = name.as_bytes();
        let crc = 0u32; // never checked here
        // Local header.
        out.extend_from_slice(&LOCAL_SIGNATURE);
        out.extend_from_slice(&[10, 0]); // version needed
        out.extend_from_slice(&[0, 0]); // flags
        out.extend_from_slice(&[0, 0]); // method: stored
        out.extend_from_slice(&[0, 0, 0, 0]); // time, date
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&(body.len() as u32).to_le_bytes());
        out.extend_from_slice(&(body.len() as u32).to_le_bytes());
        out.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        out.extend_from_slice(&[0, 0]); // extra len
        out.extend_from_slice(name_bytes);
        let data_at = out.len();
        out.extend_from_slice(body);
        let central_at = out.len();
        // Central directory.
        out.extend_from_slice(&CENTRAL_SIGNATURE);
        out.extend_from_slice(&[10, 0]); // version made by
        out.extend_from_slice(&[10, 0]); // version needed
        out.extend_from_slice(&[0, 0]); // flags
        out.extend_from_slice(&[0, 0]); // method
        out.extend_from_slice(&[0, 0, 0, 0]); // time, date
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&(body.len() as u32).to_le_bytes());
        out.extend_from_slice(&(body.len() as u32).to_le_bytes());
        out.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        out.extend_from_slice(&[0, 0]); // extra
        out.extend_from_slice(&[0, 0]); // comment
        out.extend_from_slice(&[0, 0]); // disk
        out.extend_from_slice(&[0, 0]); // internal attrs
        out.extend_from_slice(&[0, 0, 0, 0]); // external attrs
        out.extend_from_slice(&(data_at as u32 - 30 - name_bytes.len() as u32).to_le_bytes());
        out.extend_from_slice(name_bytes);
        let central_size = out.len() - central_at;
        // End of central directory.
        out.extend_from_slice(&EOCD_SIGNATURE);
        out.extend_from_slice(&[0, 0]); // this disk
        out.extend_from_slice(&[0, 0]); // disk with directory
        out.extend_from_slice(&1u16.to_le_bytes()); // entries here
        out.extend_from_slice(&1u16.to_le_bytes()); // entries total
        out.extend_from_slice(&(central_size as u32).to_le_bytes());
        out.extend_from_slice(&(central_at as u32).to_le_bytes());
        out.extend_from_slice(&[0, 0]); // comment length
        out
    }
}

#[cfg(test)]
mod tests {
    use super::tests_support::stored_zip;
    use super::*;

    /// A one-member zip built by hand: `stored`, so no compressor is needed.
    fn stored_zip_local(name: &str, body: &[u8]) -> Vec<u8> {
        stored_zip(name, body)
    }

    #[test]
    fn a_stored_member_reads_back() {
        let zip = stored_zip_local("MAIN.BMP", b"pretend this is a bitmap");
        let archive = Archive::open(zip).expect("opens");
        assert!(archive.contains("main.bmp"));
        // Skins disagree about case and wrap things in folders; both find it.
        assert!(archive.contains("MAIN.BMP"));
        assert!(archive.contains("Some Skin/main.bmp"));
        let body = archive
            .read("main.bmp")
            .expect("present")
            .expect("inflates");
        assert_eq!(body, b"pretend this is a bitmap");
        assert!(archive.read("titlebar.bmp").is_none());
    }

    #[test]
    fn a_deflated_member_reads_back() {
        use flate2::{Compression, write::DeflateEncoder};
        use std::io::Write as _;
        let body: Vec<u8> = (0..2000u32).map(|i| (i % 251) as u8).collect();
        let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&body).unwrap();
        let deflated = encoder.finish().unwrap();
        // Same layout as `stored_zip`, with method 8 and both sizes set.
        let mut zip = stored_zip_local("VISCOLOR.TXT", &deflated);
        // Patch the method and the uncompressed size in both headers.
        let name_len = "VISCOLOR.TXT".len();
        zip[8] = 8;
        let local_uncompressed = 22;
        zip[local_uncompressed..local_uncompressed + 4]
            .copy_from_slice(&(body.len() as u32).to_le_bytes());
        let central = 30 + name_len + deflated.len();
        zip[central + 10] = 8;
        let central_uncompressed = central + 24;
        zip[central_uncompressed..central_uncompressed + 4]
            .copy_from_slice(&(body.len() as u32).to_le_bytes());
        let archive = Archive::open(zip).expect("opens");
        let read = archive
            .read("viscolor.txt")
            .expect("present")
            .expect("inflates");
        assert_eq!(read, body);
    }

    #[test]
    fn junk_is_rejected_rather_than_panicking() {
        assert!(Archive::open(vec![]).is_err());
        assert!(Archive::open(vec![0; 100]).is_err());
        assert!(Archive::open(b"PK\x05\x06 not really".to_vec()).is_err());
    }

    #[test]
    fn leaf_names_ignore_folders_and_case() {
        assert_eq!(leaf_name("MAIN.BMP"), "main.bmp");
        assert_eq!(leaf_name("Skin/Sub/Main.Bmp"), "main.bmp");
        assert_eq!(leaf_name("Skin\\Windows\\CBUTTONS.BMP"), "cbuttons.bmp");
    }
}
