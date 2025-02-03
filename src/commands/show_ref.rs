use std::collections::BTreeMap;
use std::fs::{read_dir, File};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use anyhow::Context;
use clap::Args;

use crate::commands::CommandArgs;
use crate::utils::git_dir;

impl CommandArgs for ShowRefArgs {
    fn run<W>(self, writer: &mut W) -> anyhow::Result<()>
    where
        W: Write,
    {
        let git_dir = git_dir()?;
        // Map of ref paths to their hashes, a BTreeMap is used
        // to ensure the output is sorted by the ref paths
        let mut refs = BTreeMap::<PathBuf, [u8; 40]>::new();

        // Clamp the abbrev and hash values to be between 4 and 40
        let abbrev = self.abbrev.clamp(4, 40);
        let hash_limit = self.hash.map(|n| n.clamp(4, 40));

        // Read the refs based on the flags
        if self.heads {
            read_refs(&git_dir, "refs/heads", &mut refs)?;
        }
        if self.tags {
            read_refs(&git_dir, "refs/tags", &mut refs)?;
        }
        if !self.heads && !self.tags {
            read_refs(&git_dir, "refs/heads", &mut refs)?;
            read_refs(&git_dir, "refs/tags", &mut refs)?;
            read_refs(&git_dir, "refs/remotes", &mut refs)?;
            add_ref_if_exists(&git_dir, "refs/stash", &mut refs)?;
        }
        if self.head {
            read_head(&git_dir, &mut refs)?;
        }

        let refs = refs
            .into_iter()
            .map(|(path, hash)| {
                // If hash_limit is set, only show the first n characters of the hash
                // and nothing else
                if let Some(hash_limit) = hash_limit {
                    return hash[0..hash_limit].to_vec();
                }
                // If abbrev is set, show the first n characters of the hash
                // followed by a space and the path (from refs)
                let mut entry = hash[0..abbrev].to_vec();
                entry.push(b' ');
                entry.extend_from_slice(path.to_string_lossy().as_bytes());
                entry
            })
            .collect::<Vec<Vec<u8>>>()
            .join(&b'\n');

        writer.write_all(refs.as_slice()).context("write to stdout")
    }
}

/// Recursively read all refs in a directory
/// and add them to the refs map.
///
/// # Arguments
///
/// * `git_dir` - The path to the .git directory
/// * `subdir` - The subdirectory to read refs from, relative to `git_dir`
/// * `refs` - The map to add the refs to
fn read_refs(
    git_dir: &Path,
    subdir: &str,
    refs: &mut BTreeMap<PathBuf, [u8; 40]>,
) -> anyhow::Result<()> {
    for entry in read_dir(git_dir.join(subdir))? {
        let ref_path = entry?.path();
        if ref_path.is_dir() {
            read_refs(git_dir, &ref_path.to_string_lossy(), refs)?;
        } else {
            add_ref(git_dir, &ref_path, refs)?;
        }
    }
    Ok(())
}

/// Add a ref to the refs map if the file exists.
///
/// # Arguments
///
/// * `git_dir` - The path to the .git directory
/// * `sub_path` - The path to the ref file, relative to `git_dir`
/// * `refs` - The map to add the ref to
fn add_ref_if_exists(
    git_dir: &Path,
    sub_path: &str,
    refs: &mut BTreeMap<PathBuf, [u8; 40]>,
) -> anyhow::Result<()> {
    let ref_path = git_dir.join(sub_path);
    if ref_path.exists() {
        add_ref(git_dir, &ref_path, refs)?;
    }
    Ok(())
}

/// Add a ref to the refs map.
///
/// # Arguments
///
/// * `git_dir` - The path to the .git directory
/// * `path` - The path to the ref file
/// * `refs` - The map to add the ref to
fn add_ref(
    git_dir: &Path,
    path: &Path,
    refs: &mut BTreeMap<PathBuf, [u8; 40]>,
) -> anyhow::Result<()> {
    let mut file = File::open(path)?;
    let mut hash = [0; 40];
    file.read_exact(&mut hash)?;

    let stripped_path = path.strip_prefix(git_dir)?;
    refs.insert(stripped_path.to_path_buf(), hash);
    Ok(())
}

/// Read the HEAD file and add it to the refs map.
///
/// # Arguments
///
/// * `git_dir` - The path to the .git directory
/// * `refs` - The map to add the HEAD ref to
fn read_head(git_dir: &Path, refs: &mut BTreeMap<PathBuf, [u8; 40]>) -> anyhow::Result<()> {
    let head_path = git_dir.join("HEAD");
    let mut head = File::open(head_path)?;
    let mut head_path = Vec::new();

    head.seek(SeekFrom::Start(5))?; // Skip the "ref: " prefix
    head.read_to_end(&mut head_path)?;
    head_path.pop(); // Remove the trailing newline

    let head_path = PathBuf::from(std::str::from_utf8(&head_path)?);
    // If refs/heads was read, we don't need to re-read the HEAD file
    if let Some(&hash) = refs.get(&head_path) {
        refs.insert(PathBuf::from("HEAD"), hash);
        return Ok(());
    }

    let mut head = File::open(git_dir.join(head_path))?;
    let mut hash = [0; 40];
    head.read_exact(&mut hash)?;
    refs.insert(PathBuf::from("HEAD"), hash);
    Ok(())
}

#[derive(Args, Debug)]
pub(crate) struct ShowRefArgs {
    /// show the HEAD reference, even if it would be filtered out
    #[arg(long)]
    head: bool,
    /// only show heads (can be combined with tags)
    #[arg(long)]
    heads: bool,
    /// only show tags (can be combined with heads)
    #[arg(long)]
    tags: bool,
    /// only show SHA1 hash using <n> digits
    #[arg(short = 's', long, value_name = "n")]
    hash: Option<usize>,
    /// use <n> digits to display object names
    #[arg(long, value_name = "n", default_value = "40")]
    abbrev: usize,
}
