use crate::commands::git_dir;

use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::unix::fs::MetadataExt;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Context;
use sha1::{Digest, Sha1};

const INDEX_SIGNATURE: &[u8] = b"DIRC";
const INDEX_VERSION: u32 = 2;

/// Struct to represent an index entry
#[derive(Debug)]
struct IndexEntry {
    /// Creation time (seconds)
    ctime_secs: u32,
    /// Creation time (nanoseconds)
    ctime_nanos: u32,
    /// Modification time (seconds)
    mtime_secs: u32,
    /// Modification time (nanoseconds)
    mtime_nanos: u32,
    /// Device ID
    dev: u32,
    /// Inode number
    ino: u32,
    /// File mode
    mode: u32,
    /// User ID
    uid: u32,
    /// Group ID
    gid: u32,
    /// File size
    size: u16,
    /// Assume-valid flag (state of `git update-index --assume-unchanged`)
    assume_valid: bool,
    /// Stage (during merge)
    stage: MergeStatus,
    /// SHA-1 hash of the file content
    sha1: [u8; 20],
    /// Path of the file
    path: String,
}

/// Get the current UNIX timestamp as a tuple of seconds and nanoseconds
fn current_unix_time() -> (u32, u32) {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards");

    (now.as_secs() as u32, now.subsec_nanos())
}

/// Compute SHA-1 hash of file content
fn compute_sha1(path: &PathBuf) -> anyhow::Result<[u8; 20]> {
    let mut file = File::open(path)?;
    let mut hasher = Sha1::new();
    let mut buf = [0; 4096]; // Read in chunks of 4KB

    loop {
        let bytes_read = file.read(&mut buf).context("read file chunk")?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buf[..bytes_read]);
    }

    let result = hasher.finalize();
    Ok(result.into())
}

fn read_git_index() -> anyhow::Result<Vec<IndexEntry>> {
    let index_path = git_dir()?.join("index");
    let mut index_file = File::open(&index_path).context("open index file")?;

    // Read and verify the header
    let mut header = [0; 12];
    index_file
        .read_exact(&mut header)
        .context("read index header")?;

    // Check the signature and version
    if &header[0..4] != INDEX_SIGNATURE {
        anyhow::bail!("invalid index file signature");
    }
    if u32::from_be_bytes([header[4], header[5], header[6], header[7]]) != INDEX_VERSION {
        anyhow::bail!("unsupported index version");
    }

    // Read the number of entries
    let entry_count = u32::from_be_bytes([header[8], header[9], header[10], header[11]]);

    // Read entries
    let mut entries = Vec::new();
    for _ in 0..entry_count {
        let mut entry = IndexEntry {
            ctime_secs: read_u32_be(&mut index_file)?,
            ctime_nanos: read_u32_be(&mut index_file)?,
            mtime_secs: read_u32_be(&mut index_file)?,
            mtime_nanos: read_u32_be(&mut index_file)?,
            dev: read_u32_be(&mut index_file)?,
            ino: read_u32_be(&mut index_file)?,
            mode: read_u32_be(&mut index_file)?,
            uid: read_u32_be(&mut index_file)?,
            gid: read_u32_be(&mut index_file)?,
            size: read_u16_be(&mut index_file)?,
            sha1: read_n_be(&mut index_file)?,
            assume_valid: false,
            stage: MergeStatus::Base,
            path: String::new(),
        };

        let flags = read_n_be::<2>(&mut index_file)?;
        // Read the most significant bit of the first byte to get the assume-valid flag
        entry.assume_valid = flags[0] >> 7 & 1 == 1;
        // Read the 3rd and 4th most significant bits of the first byte to get the stage
        entry.stage = MergeStatus::try_from(flags[0] >> 4 & 3)?;
        // Read the 12 least significant bits of the first byte and the second byte to get the length of the path
        let path_len = (((flags[0] & 0x0F) as u16) << 8 | flags[1] as u16) as usize;

        index_file
            .seek(SeekFrom::Current(2))
            .context("skip 2 extended flag bytes")?;

        // Read the path
        let mut path = vec![0; path_len];
        index_file
            .read_exact(&mut path)
            .with_context(|| format!("read {} bytes for path", path_len))?;
        entry.path = String::from_utf8(path).context("parse path")?;

        // Skip null padding
        let null_padding = calc_null_padding(path_len);
        index_file
            .seek(SeekFrom::Current(null_padding as i64))
            .with_context(|| format!("skip {} null padding bytes", null_padding))?;

        entries.push(entry);
    }

    // Read the trailing checksum
    let mut checksum = [0; 20];
    index_file
        .read_exact(&mut checksum)
        .context("read trailing checksum")?;
    let _computed_checksum = compute_sha1(&index_path)?;

    // TODO: Figure out
    // if checksum != computed_checksum {
    //     anyhow::bail!("index file checksum mismatch");
    // }

    Ok(entries)
}

fn read_u32_be(file: &mut File) -> anyhow::Result<u32> {
    let mut buf = [0; 4];
    file.read_exact(&mut buf).context("read 4 bytes")?;
    Ok(u32::from_be_bytes(buf))
}

fn read_u16_be(file: &mut File) -> anyhow::Result<u16> {
    let mut buf = [0; 2];
    file.read_exact(&mut buf).context("read 2 bytes")?;
    Ok(u16::from_be_bytes(buf))
}

fn read_n_be<const N: usize>(file: &mut File) -> anyhow::Result<[u8; N]> {
    let mut buf = [0; N];
    file.read_exact(&mut buf)
        .with_context(|| format!("read {} bytes", N))?;
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn temp() {
        println!("{:#?}", read_git_index().unwrap());
        panic!();
    }
}

/// Write the Git index file
fn write_git_index(file_paths: &[PathBuf]) -> anyhow::Result<()> {
    let mut entries = Vec::new();

    for path in file_paths {
        let metadata = path.metadata().context("read file metadata")?;
        let sha1 = compute_sha1(path)?;
        let now = current_unix_time();

        let entry = IndexEntry {
            ctime_secs: now.0,
            ctime_nanos: now.1,
            mtime_secs: now.0,
            mtime_nanos: now.1,
            dev: metadata.dev() as u32,
            ino: metadata.ino() as u32,
            mode: metadata.mode(),
            uid: metadata.uid(),
            gid: metadata.gid(),
            size: metadata.len() as u16,
            sha1,
            assume_valid: false,
            stage: MergeStatus::Unmerged,
            path: path.to_string_lossy().to_string(),
        };
        entries.push(entry);
    }

    let index_path = git_dir()?.join("index");
    let mut file = File::create(&index_path).context("create index file")?;

    // Write the header
    file.write_all(b"DIRC")?; // Signature
    file.write_all(&2u32.to_be_bytes())?; // Version
    file.write_all(&(entries.len() as u32).to_be_bytes())?; // Entry count

    // Write entries
    for entry in entries {
        file.write_all(&entry.ctime_secs.to_be_bytes())?;
        file.write_all(&entry.ctime_nanos.to_be_bytes())?;
        file.write_all(&entry.mtime_secs.to_be_bytes())?;
        file.write_all(&entry.mtime_nanos.to_be_bytes())?;
        file.write_all(&entry.dev.to_be_bytes())?;
        file.write_all(&entry.ino.to_be_bytes())?;
        file.write_all(&entry.mode.to_be_bytes())?;
        file.write_all(&entry.uid.to_be_bytes())?;
        file.write_all(&entry.gid.to_be_bytes())?;
        file.write_all(&entry.size.to_be_bytes())?;
        file.write_all(&entry.sha1)?;

        let mut flags = [0b0000_0000, 0b0000_0000];
        flags[0] |= (entry.path.len() as u16 >> 8) as u8 & 0x0F;
        flags[1] |= entry.path.len() as u8 & 0xFF;

        file.write_all(&flags)?;
        file.write_all(&0u16.to_be_bytes())?; // Extended flags
        file.write_all(entry.path.as_bytes())?;

        let null_padding = calc_null_padding(entry.path.len());
        file.write_all(&vec![0; null_padding])?; // Null padding
    }

    // Compute and write the trailing checksum
    file.sync_all()?;
    // TODO: Figure out
    let checksum = compute_sha1(&index_path)?;
    file.write_all(&checksum)?;

    Ok(())
}

fn calc_null_padding(path_len: usize) -> usize {
    8 - (6 + path_len) % 8
}

/// Enum to represent the status of a file during a merge
#[derive(Debug)]
enum MergeStatus {
    /// The file is unmerged
    Unmerged,
    /// The file is merged and the content is the same as the base
    Base,
    /// The file is merged and the content is the same as ours
    Ours,
    /// The file is merged and the content is the same as theirs
    Theirs,
}

impl TryFrom<u8> for MergeStatus {
    type Error = anyhow::Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(MergeStatus::Unmerged),
            1 => Ok(MergeStatus::Base),
            2 => Ok(MergeStatus::Ours),
            3 => Ok(MergeStatus::Theirs),
            _ => anyhow::bail!("invalid merge status"),
        }
    }
}
