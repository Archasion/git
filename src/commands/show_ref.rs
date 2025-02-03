use std::collections::BTreeMap;
use std::fs::File;
use std::io::{Read, Write};
use std::path::PathBuf;

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
        let ref_dir = git_dir.join("refs");

        // use a BTreeMap to sort the entries by path
        // the entries are stored as a key-value pair of the path and the hash
        let mut refs = BTreeMap::<PathBuf, [u8; 40]>::new();
        read_refs(&git_dir, ref_dir, &mut refs)?;

        let refs = refs
            .into_iter()
            .map(|(path, hash)| {
                let mut entry = hash.to_vec();
                let path = path.to_string_lossy();

                // format the entries as "<hash> <path>"
                entry.push(b' ');
                entry.extend_from_slice(path.as_bytes());
                entry
            })
            .collect::<Vec<_>>()
            .join(&b'\n');

        writer.write_all(refs.as_slice()).context("write to stdout")
    }
}

/// Recursively read all reference files in the given directory.
///
/// # Arguments
///
/// * `git_dir` - The path to the git directory
/// * `ref_dir` - The path to the directory containing the references
/// * `refs` - A mutable reference to a [`BTreeMap`] to store the references
fn read_refs(
    git_dir: &PathBuf,
    ref_dir: PathBuf,
    refs: &mut BTreeMap<PathBuf, [u8; 40]>,
) -> anyhow::Result<()> {
    let entries = std::fs::read_dir(ref_dir)?;
    for entry in entries {
        let ref_path = entry?.path();
        // recurse into subdirectories
        if ref_path.is_dir() {
            read_refs(git_dir, ref_path, refs)?;
            continue;
        }

        let mut file = File::open(&ref_path)?;
        let mut hash = [0; 40];
        // read 40-byte hex hash
        file.read_exact(&mut hash)?;

        // remove the git directory prefix from the path
        let ref_path = ref_path
            .strip_prefix(git_dir.as_path())
            .context("strip prefix")?
            .to_path_buf();
        refs.insert(ref_path, hash);
    }
    Ok(())
}

#[derive(Args, Debug)]
pub(crate) struct ShowRefArgs {
    /// show the HEAD reference, even if it would be filtered out
    #[arg(long)]
    head: bool,
    /// only show branches (can be combined with tags)
    #[arg(long)]
    branches: bool,
    /// only show tags (can be combined with branches)
    #[arg(long)]
    tags: bool,
    /// stricter reference checking, requires exact ref path
    #[arg(long, requires = "pattern")]
    verify: bool,
    /// dereference tags into object IDs
    #[arg(short, long)]
    dereference: bool,
    /// only show SHA1 hash using <n> digits
    #[arg(short = 's', long, value_name = "n")]
    hash: Option<usize>,
    /// use <n> digits to display object names
    #[arg(long, value_name = "n")]
    abbrev: Option<usize>,
    /// do not print results to stdout (useful with --verify)
    #[arg(short, long)]
    quiet: bool,
    /// show refs from stdin that aren't in local repository
    #[arg(long, value_name = "pattern", conflicts_with = "pattern")]
    exclude_existing: Option<String>,
    /// only show refs that match the given pattern
    #[arg(name = "pattern", required = false)]
    patterns: Vec<String>,
}
