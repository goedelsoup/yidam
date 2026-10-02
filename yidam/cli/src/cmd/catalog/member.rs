//! A member read out of a zip artifact, as a reading of it (#1351).
//!
//! A location names the bytes a fetch reads, never a file inside them (RFC-0048 decision 9).
//! So `oh2010.sf1.zip` is fetched whole and its packing list is named beside the location
//! that fetched it, under `members:`:
//!
//! ```yaml
//! location:
//!   - kind: url
//!     value: https://www2.census.gov/census_2010/04-Summary_File_1/Ohio/oh2010.sf1.zip
//!     members:
//!       - oh2010.sf1.prd.packinglist.txt
//! ```
//!
//! `catalog-extract` unpacks each one from the cached zip and records it under the zip's
//! record as a reading with its own digest and media type, `member:` naming the path and `by:`
//! naming [`UNPACKER`]. A re-read of the same member is comparable with the first by digest.
//!
//! # What it reads
//!
//! Stored and deflated members, the two methods every zip the measured corpora name uses.
//! The member's CRC is checked, so bytes that unpack without an error but are not the bytes
//! the archive holds are refused rather than recorded.
//!
//! # What it refuses
//!
//! - a member path that is empty, absolute, holds `..`, a backslash or a drive prefix
//!   ([`refusal`]), checked before the archive is opened, and by lint before that
//! - an archive whose directory lists more than [`MAX_ENTRIES`] files
//! - a member larger than [`MAX_MEMBER_BYTES`] unpacked. The limit holds on the bytes
//!   written, not only on the size the directory declares, which an archive can understate.
//! - an encrypted member, a method other than stored or deflate, a zip64 archive, and a name
//!   the archive lists twice, since that leaves which one was read to chance

use std::fs::File;
use std::io::{BufWriter, Read, Seek, SeekFrom, Write};
use std::path::Path;

/// What a member reading's `by:` names: the step that unpacked it. It carries no version
/// because unpacking is not an interpretation. Any reader gives the same bytes, and the
/// reading's digest is what a re-read is compared with.
pub(crate) const UNPACKER: &str = "unzip";

/// The most files an archive's directory may list. The largest measured, a census summary
/// file, lists 49.
pub(crate) const MAX_ENTRIES: usize = 10_000;

/// The largest member this reads, unpacked: 1 GiB. A census summary file segment is about
/// 90 MB open.
pub(crate) const MAX_MEMBER_BYTES: u64 = 1 << 30;

const EOCD: u32 = 0x0605_4b50;
const ZIP64_LOCATOR: u32 = 0x0706_4b50;
const CENTRAL: u32 = 0x0201_4b50;
const LOCAL: u32 = 0x0403_4b50;

/// What is wrong with `member` as a path inside an archive, if anything.
pub(crate) fn refusal(member: &str) -> Option<String> {
    let why = if member.trim().is_empty() {
        "is empty"
    } else if member.starts_with('/') {
        "is absolute"
    } else if member.contains('\\') {
        "holds a backslash, and a zip separates names with `/`"
    } else if member.as_bytes().get(1) == Some(&b':') && member.as_bytes()[0].is_ascii_alphabetic()
    {
        "starts with a drive prefix"
    } else if member.split('/').any(|c| c == "..") {
        "climbs out of the archive with `..`"
    } else if member.ends_with('/') {
        "names a directory, not a file"
    } else if member.contains('\0') {
        "holds a NUL byte"
    } else {
        return None;
    };
    Some(format!("member `{member}` {why}"))
}

/// The media type a member's reading records, by its extension. A zip states none for its
/// members, so the name is all there is, and a name this does not know is
/// `application/octet-stream` rather than a guess.
pub(crate) fn media_type(member: &str) -> &'static str {
    let ext = member
        .rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "txt" => "text/plain",
        "csv" => "text/csv",
        "tsv" | "tab" => "text/tab-separated-values",
        "json" => "application/json",
        "xml" => "application/xml",
        "html" | "htm" => "text/html",
        "pdf" => "application/pdf",
        "xls" => "application/vnd.ms-excel",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "dbf" => "application/x-dbf",
        "zip" => "application/zip",
        _ => "application/octet-stream",
    }
}

/// Whether a record's media type says the bytes may be a zip. A record that states none is
/// read as one, and the archive's own structure decides.
pub(crate) fn may_be_zip(media: Option<&str>) -> bool {
    media.is_none_or(|m| {
        let m = m.split(';').next().unwrap_or(m).trim();
        [
            "application/zip",
            "application/x-zip-compressed",
            "application/x-zip",
        ]
        .iter()
        .any(|z| m.eq_ignore_ascii_case(z))
    })
}

/// One directory entry, as much of it as reading a member needs.
struct Entry {
    name: Vec<u8>,
    flags: u16,
    method: u16,
    crc: u32,
    compressed: u64,
    size: u64,
    local: u64,
}

/// Unpack `member` from the zip at `archive` into `to`, and return its size, or why not.
pub(crate) fn unpack(archive: &Path, member: &str, to: &Path) -> Result<u64, String> {
    if let Some(why) = refusal(member) {
        return Err(why);
    }
    let mut f = File::open(archive).map_err(|e| format!("could not be opened: {e}"))?;
    let entries = directory(&mut f)?;
    let mut named = entries.iter().filter(|e| e.name == member.as_bytes());
    let entry = match (named.next(), named.next()) {
        (None, _) => return Err(format!("holds no member `{member}`")),
        (Some(_), Some(_)) => return Err(format!("lists member `{member}` twice")),
        (Some(e), None) => e,
    };
    if entry.flags & 1 != 0 {
        return Err(format!("encrypts member `{member}`"));
    }
    if entry.size > MAX_MEMBER_BYTES {
        return Err(too_large(member));
    }

    let local = read_at(&mut f, entry.local, 30)?;
    if le32(&local, 0) != LOCAL {
        return Err(format!(
            "member `{member}` has no local header where the directory says"
        ));
    }
    let data = entry.local + 30 + u64::from(le16(&local, 26)) + u64::from(le16(&local, 28));
    f.seek(SeekFrom::Start(data))
        .map_err(|e| format!("could not be read: {e}"))?;
    let raw = f.take(entry.compressed);

    let out = File::create(to).map_err(|e| format!("could not stage `{member}`: {e}"))?;
    let mut sink = Checked {
        out: BufWriter::new(out),
        crc: flate2::Crc::new(),
        written: 0,
    };
    // One byte past the limit, so a member that understates its size is caught by what it
    // writes rather than by what it declared.
    let copied = match entry.method {
        0 => std::io::copy(&mut raw.take(MAX_MEMBER_BYTES + 1), &mut sink),
        8 => std::io::copy(
            &mut flate2::read::DeflateDecoder::new(raw).take(MAX_MEMBER_BYTES + 1),
            &mut sink,
        ),
        m => {
            return Err(format!(
                "packs member `{member}` by method {m}, which is neither stored nor deflate"
            ))
        }
    };
    copied.map_err(|e| format!("member `{member}` did not unpack: {e}"))?;
    sink.out
        .flush()
        .map_err(|e| format!("could not stage `{member}`: {e}"))?;
    if sink.written > MAX_MEMBER_BYTES {
        return Err(too_large(member));
    }
    if sink.written != entry.size || sink.crc.sum() != entry.crc {
        return Err(format!(
            "member `{member}` unpacked to bytes that do not match the archive's own size and CRC"
        ));
    }
    Ok(sink.written)
}

fn too_large(member: &str) -> String {
    format!(
        "member `{member}` is larger than {} MiB unpacked, which is the most this reads",
        MAX_MEMBER_BYTES >> 20
    )
}

/// A writer that counts and checksums what passes through it.
struct Checked<W> {
    out: W,
    crc: flate2::Crc,
    written: u64,
}

impl<W: Write> Write for Checked<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let n = self.out.write(buf)?;
        self.crc.update(&buf[..n]);
        self.written += n as u64;
        Ok(n)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.out.flush()
    }
}

/// Every entry the archive's central directory lists.
fn directory(f: &mut File) -> Result<Vec<Entry>, String> {
    let len = f
        .seek(SeekFrom::End(0))
        .map_err(|e| format!("could not be read: {e}"))?;
    // The end record is 22 bytes and may be followed by a comment of up to 65,535.
    let tail_len = len.min(22 + 0xFFFF);
    let tail = read_at(f, len - tail_len, tail_len as usize)?;
    let at = (0..tail.len().saturating_sub(21))
        .rev()
        .find(|&i| le32(&tail, i) == EOCD)
        .ok_or("is not a zip archive: it has no end-of-directory record")?;
    if at >= 20 && le32(&tail, at - 20) == ZIP64_LOCATOR {
        return Err("is a zip64 archive, which this reader does not read".into());
    }
    let count = usize::from(le16(&tail, at + 10));
    let size = u64::from(le32(&tail, at + 12));
    let offset = u64::from(le32(&tail, at + 16));
    if count > MAX_ENTRIES {
        return Err(format!(
            "lists {count} files, and the most this reads is {MAX_ENTRIES}"
        ));
    }
    if offset.saturating_add(size) > len {
        return Err("is truncated: its directory runs past the end of the file".into());
    }
    let dir = read_at(f, offset, size as usize)?;

    let mut entries = Vec::with_capacity(count);
    let mut i = 0usize;
    for _ in 0..count {
        if i + 46 > dir.len() || le32(&dir, i) != CENTRAL {
            return Err("has a directory that does not parse".into());
        }
        let (n, x, c) = (
            usize::from(le16(&dir, i + 28)),
            usize::from(le16(&dir, i + 30)),
            usize::from(le16(&dir, i + 32)),
        );
        let name = dir
            .get(i + 46..i + 46 + n)
            .ok_or("has a directory that does not parse")?
            .to_vec();
        let (compressed, size, local) =
            (le32(&dir, i + 20), le32(&dir, i + 24), le32(&dir, i + 42));
        if [compressed, size, local].contains(&u32::MAX) {
            return Err("is a zip64 archive, which this reader does not read".into());
        }
        entries.push(Entry {
            name,
            flags: le16(&dir, i + 8),
            method: le16(&dir, i + 10),
            crc: le32(&dir, i + 16),
            compressed: u64::from(compressed),
            size: u64::from(size),
            local: u64::from(local),
        });
        i += 46 + n + x + c;
    }
    Ok(entries)
}

fn read_at(f: &mut File, at: u64, len: usize) -> Result<Vec<u8>, String> {
    let mut buf = vec![0u8; len];
    f.seek(SeekFrom::Start(at))
        .and_then(|_| f.read_exact(&mut buf))
        .map_err(|_| "is truncated".to_string())?;
    Ok(buf)
}

fn le16(b: &[u8], at: usize) -> u16 {
    b.get(at..at + 2)
        .map_or(0, |s| u16::from_le_bytes([s[0], s[1]]))
}

fn le32(b: &[u8], at: usize) -> u32 {
    b.get(at..at + 4)
        .map_or(0, |s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

/// A zip written here, so the tests state the bytes they read: each `(name, bytes, deflate)`.
#[cfg(test)]
pub(crate) fn zip(members: &[(&str, &[u8], bool)]) -> Vec<u8> {
    let (mut out, mut dir) = (Vec::new(), Vec::new());
    for (name, bytes, deflate) in members {
        let mut crc = flate2::Crc::new();
        crc.update(bytes);
        let body = if *deflate {
            let mut e =
                flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
            let _ = e.write_all(bytes);
            e.finish().unwrap_or_default()
        } else {
            bytes.to_vec()
        };
        let method: u16 = if *deflate { 8 } else { 0 };
        let local = out.len() as u32;
        let fields = |sig: u32| {
            let mut h = sig.to_le_bytes().to_vec();
            h.extend(20u16.to_le_bytes());
            if sig == CENTRAL {
                h.extend(20u16.to_le_bytes());
            }
            h.extend(0u16.to_le_bytes());
            h.extend(method.to_le_bytes());
            h.extend([0u8; 4]);
            h.extend(crc.sum().to_le_bytes());
            h.extend((body.len() as u32).to_le_bytes());
            h.extend((bytes.len() as u32).to_le_bytes());
            h.extend((name.len() as u16).to_le_bytes());
            h.extend(0u16.to_le_bytes());
            h
        };
        out.extend(fields(LOCAL));
        out.extend(name.as_bytes());
        out.extend(&body);
        dir.extend(fields(CENTRAL));
        dir.extend([0u8; 10]);
        dir.extend(local.to_le_bytes());
        dir.extend(name.as_bytes());
    }
    let at = out.len() as u32;
    out.extend(&dir);
    out.extend(EOCD.to_le_bytes());
    out.extend([0u8; 4]);
    out.extend((members.len() as u16).to_le_bytes());
    out.extend((members.len() as u16).to_le_bytes());
    out.extend((dir.len() as u32).to_le_bytes());
    out.extend(at.to_le_bytes());
    out.extend(0u16.to_le_bytes());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn staged(archive: &[u8]) -> (tempfile::TempDir, std::path::PathBuf, std::path::PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        let a = tmp.path().join("a.zip");
        std::fs::write(&a, archive).unwrap();
        let to = tmp.path().join("out");
        (tmp, a, to)
    }

    #[test]
    fn a_stored_and_a_deflated_member_unpack_to_their_bytes() {
        let list = b"P1|1|1\nH3|44|3\n".repeat(50);
        let a = zip(&[
            ("oh2010.sf1.prd.packinglist.txt", &list, true),
            ("readme.txt", b"stored as is", false),
        ]);
        let (_tmp, at, to) = staged(&a);
        assert_eq!(
            unpack(&at, "oh2010.sf1.prd.packinglist.txt", &to),
            Ok(list.len() as u64)
        );
        assert_eq!(std::fs::read(&to).unwrap(), list);
        assert_eq!(unpack(&at, "readme.txt", &to), Ok(12));
        assert_eq!(std::fs::read(&to).unwrap(), b"stored as is");
    }

    #[test]
    fn a_member_the_archive_does_not_hold_or_holds_twice_is_refused() {
        let (_tmp, at, to) = staged(&zip(&[("a.txt", b"1", false), ("a.txt", b"2", false)]));
        assert!(unpack(&at, "b.txt", &to)
            .unwrap_err()
            .contains("holds no member"));
        assert!(unpack(&at, "a.txt", &to).unwrap_err().contains("twice"));
    }

    #[test]
    fn bytes_that_are_not_a_zip_or_do_not_match_their_crc_are_refused() {
        let (_tmp, at, to) = staged(b"%PDF-1.4 not an archive");
        assert!(unpack(&at, "a.txt", &to).unwrap_err().contains("not a zip"));

        let mut a = zip(&[("a.txt", b"abc", false)]);
        a[30 + 5] = b'X';
        let (_tmp, at, to) = staged(&a);
        assert!(unpack(&at, "a.txt", &to).unwrap_err().contains("CRC"));
    }

    #[test]
    fn a_path_that_leaves_the_archive_is_refused_before_it_is_opened() {
        for bad in [
            "",
            "/etc/passwd",
            "../x",
            "a/../../x",
            "a\\b",
            "C:x",
            "dir/",
        ] {
            assert!(refusal(bad).is_some(), "{bad:?}");
            let why = unpack(Path::new("/nonexistent.zip"), bad, Path::new("/x")).unwrap_err();
            assert!(why.starts_with("member"), "{bad:?}: {why}");
        }
        for good in ["a.txt", "dir/a.txt", "a..b.txt"] {
            assert_eq!(refusal(good), None, "{good:?}");
        }
    }

    #[test]
    fn a_member_names_its_media_type_by_extension_and_only_a_zip_record_is_read() {
        assert_eq!(media_type("x/PACKINGLIST.TXT"), "text/plain");
        assert_eq!(media_type("MSA_M2024_dl.xlsx"), media_type("a.xlsx"));
        assert_eq!(media_type("ohgeo2020.pl"), "application/octet-stream");
        assert!(may_be_zip(None));
        assert!(may_be_zip(Some("application/zip")));
        assert!(!may_be_zip(Some("application/pdf")));
    }
}
