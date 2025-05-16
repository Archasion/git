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
    let subdir_path = git_dir.join(subdir);

    if !subdir_path.exists() {
        return Ok(());
    }

    for entry in read_dir(subdir_path)? {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::init::get_head_ref_content;
    use crate::utils::env;
    use crate::utils::test::{TempEnv, TempPwd};

    const HEAD_HASH: &str = "aabbccddeeff00112233445566778899aabbccdd";
    const HEAD_NAME: &str = "main";
    const TAG_HASH: &str = "112233445566778899aabbccddeeff0011223344";
    const TAG_NAME: &str = "v1.0";
    const REMOTE_HASH: &str = "33445566778899aabbccddeeff00112233445566";
    const REMOTE_NAME: &str = "origin";
    const STASH_HASH: &str = "5566778899aabbccddeeff001122334455667788";

    // Head can be excluded from the enum as it must always be present
    struct Ref {
        dir: &'static str,
        name: &'static str,
        hash: &'static [u8],
    }

    /// Create a temporary `.git/refs` directory with refs of the specified types.
    ///
    /// The `stash` and `HEAD` refs are always created.
    fn create_temp_refs<const N: usize>(refs: [Ref; N]) -> TempPwd {
        let _env = TempEnv::unset(env::GIT_DIR);
        let pwd = TempPwd::new();
        let git_dir = pwd.path().join(".git");
        let refs_dir = git_dir.join("refs");

        std::fs::create_dir_all(&refs_dir).unwrap();

        for Ref { dir, name, hash } in refs {
            let ref_dir = refs_dir.join(dir);
            std::fs::create_dir(&ref_dir).unwrap();
            let ref_file = ref_dir.join(name);
            std::fs::write(&ref_file, hash).unwrap();
        }

        // Store the HEAD ref in /refs/heads
        let heads_dir = refs_dir.join("heads");
        std::fs::create_dir(&heads_dir).unwrap();
        let head_file = heads_dir.join(HEAD_NAME);
        std::fs::write(&head_file, HEAD_HASH).unwrap();

        // Create a HEAD file that points to the main branch
        let head_file = git_dir.join("HEAD");
        std::fs::write(&head_file, get_head_ref_content(HEAD_NAME)).unwrap();

        // Create a stash file
        let stash_file = refs_dir.join("stash");
        std::fs::write(&stash_file, STASH_HASH).unwrap();

        pwd
    }

    #[test]
    fn show_refs() {
        let _pwd = create_temp_refs([
            Ref {
                dir: "tags",
                name: TAG_NAME,
                hash: TAG_HASH.as_bytes(),
            },
            Ref {
                dir: "remotes",
                name: REMOTE_NAME,
                hash: REMOTE_HASH.as_bytes(),
            },
        ]);

        let args = ShowRefArgs {
            head: false,
            heads: false,
            tags: false,
            hash: None,
            abbrev: 40,
        };

        let mut output = Vec::new();
        let result = args.run(&mut output);
        let expected = format!(
            "{HEAD_HASH} refs/heads/{HEAD_NAME}\n\
             {REMOTE_HASH} refs/remotes/{REMOTE_NAME}\n\
             {STASH_HASH} refs/stash\n\
             {TAG_HASH} refs/tags/{TAG_NAME}",
        )
        .into_bytes();

        assert!(result.is_ok());
        assert_eq!(output, expected);
    }

    #[test]
    fn show_refs_with_head() {
        let _pwd = create_temp_refs([
            Ref {
                dir: "tags",
                name: TAG_NAME,
                hash: TAG_HASH.as_bytes(),
            },
            Ref {
                dir: "remotes",
                name: REMOTE_NAME,
                hash: REMOTE_HASH.as_bytes(),
            },
        ]);

        let args = ShowRefArgs {
            head: true,
            heads: false,
            tags: false,
            hash: None,
            abbrev: 40,
        };

        let mut output = Vec::new();
        let result = args.run(&mut output);
        let expected = format!(
            "{HEAD_HASH} HEAD\n\
             {HEAD_HASH} refs/heads/{HEAD_NAME}\n\
             {REMOTE_HASH} refs/remotes/{REMOTE_NAME}\n\
             {STASH_HASH} refs/stash\n\
             {TAG_HASH} refs/tags/{TAG_NAME}",
        )
        .into_bytes();

        assert!(result.is_ok());
        assert_eq!(output, expected);
    }

    #[test]
    fn show_head_refs() {
        let _pwd = create_temp_refs([
            Ref {
                dir: "tags",
                name: TAG_NAME,
                hash: TAG_HASH.as_bytes(),
            },
            Ref {
                dir: "remotes",
                name: REMOTE_NAME,
                hash: REMOTE_HASH.as_bytes(),
            },
        ]);

        let args = ShowRefArgs {
            head: false,
            heads: true,
            tags: false,
            hash: None,
            abbrev: 40,
        };

        let mut output = Vec::new();
        let result = args.run(&mut output);
        let expected = format!("{HEAD_HASH} refs/heads/{HEAD_NAME}");

        assert!(result.is_ok());
        assert_eq!(output, expected.into_bytes());
    }

    #[test]
    fn show_tag_refs() {
        let _pwd = create_temp_refs([
            Ref {
                dir: "tags",
                name: TAG_NAME,
                hash: TAG_HASH.as_bytes(),
            },
            Ref {
                dir: "remotes",
                name: REMOTE_NAME,
                hash: REMOTE_HASH.as_bytes(),
            },
        ]);

        let args = ShowRefArgs {
            head: false,
            heads: false,
            tags: true,
            hash: None,
            abbrev: 40,
        };

        let mut output = Vec::new();
        let result = args.run(&mut output);
        let expected = format!("{TAG_HASH} refs/tags/{TAG_NAME}");

        assert!(result.is_ok());
        assert_eq!(output, expected.into_bytes());
    }

    #[test]
    fn show_tag_and_head_refs() {
        let _pwd = create_temp_refs([
            Ref {
                dir: "tags",
                name: TAG_NAME,
                hash: TAG_HASH.as_bytes(),
            },
            Ref {
                dir: "remotes",
                name: REMOTE_NAME,
                hash: REMOTE_HASH.as_bytes(),
            },
        ]);

        let args = ShowRefArgs {
            head: false,
            heads: true,
            tags: true,
            hash: None,
            abbrev: 40,
        };

        let mut output = Vec::new();
        let result = args.run(&mut output);
        let expected = format!(
            "{HEAD_HASH} refs/heads/{HEAD_NAME}\n\
             {TAG_HASH} refs/tags/{TAG_NAME}",
        );

        assert!(result.is_ok());
        assert_eq!(output, expected.into_bytes());
    }

    #[test]
    fn show_tag_and_head_refs_with_head() {
        let _pwd = create_temp_refs([
            Ref {
                dir: "tags",
                name: TAG_NAME,
                hash: TAG_HASH.as_bytes(),
            },
            Ref {
                dir: "remotes",
                name: REMOTE_NAME,
                hash: REMOTE_HASH.as_bytes(),
            },
        ]);

        let args = ShowRefArgs {
            head: true,
            heads: true,
            tags: true,
            hash: None,
            abbrev: 40,
        };

        let mut output = Vec::new();
        let result = args.run(&mut output);
        let expected = format!(
            "{HEAD_HASH} HEAD\n\
             {HEAD_HASH} refs/heads/{HEAD_NAME}\n\
             {TAG_HASH} refs/tags/{TAG_NAME}",
        );

        assert!(result.is_ok());
        assert_eq!(output, expected.into_bytes());
    }

    #[test]
    fn show_tag_refs_with_head() {
        let _pwd = create_temp_refs([
            Ref {
                dir: "tags",
                name: TAG_NAME,
                hash: TAG_HASH.as_bytes(),
            },
            Ref {
                dir: "remotes",
                name: REMOTE_NAME,
                hash: REMOTE_HASH.as_bytes(),
            },
        ]);

        let args = ShowRefArgs {
            head: true,
            heads: false,
            tags: true,
            hash: None,
            abbrev: 40,
        };

        let mut output = Vec::new();
        let result = args.run(&mut output);
        let expected = format!(
            "{HEAD_HASH} HEAD\n\
             {TAG_HASH} refs/tags/{TAG_NAME}",
        );

        assert!(result.is_ok());
        assert_eq!(output, expected.into_bytes());
    }

    #[test]
    fn show_no_tag_refs() {
        let _pwd = create_temp_refs([]);
        let args = ShowRefArgs {
            head: false,
            heads: false,
            tags: true,
            hash: None,
            abbrev: 40,
        };

        let mut output = Vec::new();
        let result = args.run(&mut output);

        assert!(result.is_ok());
        assert_eq!(output, Vec::new());
    }

    #[test]
    fn abbreviate_ref_hashes() {
        let _pwd = create_temp_refs([
            Ref {
                dir: "tags",
                name: TAG_NAME,
                hash: TAG_HASH.as_bytes(),
            },
            Ref {
                dir: "remotes",
                name: REMOTE_NAME,
                hash: REMOTE_HASH.as_bytes(),
            },
        ]);

        let args = ShowRefArgs {
            head: false,
            heads: false,
            tags: false,
            hash: None,
            abbrev: 8,
        };

        let mut output = Vec::new();
        let result = args.run(&mut output);
        let expected = format!(
            "{} refs/heads/{HEAD_NAME}\n\
             {} refs/remotes/{REMOTE_NAME}\n\
             {} refs/stash\n\
             {} refs/tags/{TAG_NAME}",
            &HEAD_HASH[0..8],
            &REMOTE_HASH[0..8],
            &STASH_HASH[0..8],
            &TAG_HASH[0..8],
        )
        .into_bytes();

        assert!(result.is_ok());
        assert_eq!(output, expected);
    }

    #[test]
    fn abbreviate_ref_hashes_below_min() {
        let _pwd = create_temp_refs([
            Ref {
                dir: "tags",
                name: TAG_NAME,
                hash: TAG_HASH.as_bytes(),
            },
            Ref {
                dir: "remotes",
                name: REMOTE_NAME,
                hash: REMOTE_HASH.as_bytes(),
            },
        ]);

        let args = ShowRefArgs {
            head: false,
            heads: false,
            tags: false,
            hash: None,
            abbrev: 2,
        };

        let mut output = Vec::new();
        let result = args.run(&mut output);
        let expected = format!(
            "{} refs/heads/{HEAD_NAME}\n\
             {} refs/remotes/{REMOTE_NAME}\n\
             {} refs/stash\n\
             {} refs/tags/{TAG_NAME}",
            &HEAD_HASH[0..4],
            &REMOTE_HASH[0..4],
            &STASH_HASH[0..4],
            &TAG_HASH[0..4],
        )
        .into_bytes();

        assert!(result.is_ok());
        assert_eq!(output, expected);
    }

    #[test]
    fn abbreviate_ref_hashes_above_max() {
        let _pwd = create_temp_refs([
            Ref {
                dir: "tags",
                name: TAG_NAME,
                hash: TAG_HASH.as_bytes(),
            },
            Ref {
                dir: "remotes",
                name: REMOTE_NAME,
                hash: REMOTE_HASH.as_bytes(),
            },
        ]);

        let args = ShowRefArgs {
            head: false,
            heads: false,
            tags: false,
            hash: None,
            abbrev: 50,
        };

        let mut output = Vec::new();
        let result = args.run(&mut output);
        let expected = format!(
            "{} refs/heads/{HEAD_NAME}\n\
             {} refs/remotes/{REMOTE_NAME}\n\
             {} refs/stash\n\
             {} refs/tags/{TAG_NAME}",
            &HEAD_HASH, &REMOTE_HASH, &STASH_HASH, &TAG_HASH,
        )
        .into_bytes();

        assert!(result.is_ok());
        assert_eq!(output, expected);
    }

    #[test]
    fn show_hashes_with_limit() {
        let _pwd = create_temp_refs([
            Ref {
                dir: "tags",
                name: TAG_NAME,
                hash: TAG_HASH.as_bytes(),
            },
            Ref {
                dir: "remotes",
                name: REMOTE_NAME,
                hash: REMOTE_HASH.as_bytes(),
            },
        ]);

        let args = ShowRefArgs {
            head: false,
            heads: false,
            tags: false,
            hash: Some(8),
            abbrev: 40,
        };

        let mut output = Vec::new();
        let result = args.run(&mut output);
        let expected = format!(
            "{}\n{}\n{}\n{}",
            &HEAD_HASH[0..8],
            &REMOTE_HASH[0..8],
            &STASH_HASH[0..8],
            &TAG_HASH[0..8],
        )
        .into_bytes();

        assert!(result.is_ok());
        assert_eq!(output, expected);
    }

    #[test]
    fn show_hashes_with_limit_below_min() {
        let _pwd = create_temp_refs([
            Ref {
                dir: "tags",
                name: TAG_NAME,
                hash: TAG_HASH.as_bytes(),
            },
            Ref {
                dir: "remotes",
                name: REMOTE_NAME,
                hash: REMOTE_HASH.as_bytes(),
            },
        ]);

        let args = ShowRefArgs {
            head: false,
            heads: false,
            tags: false,
            hash: Some(2),
            abbrev: 40,
        };

        let mut output = Vec::new();
        let result = args.run(&mut output);
        let expected = format!(
            "{}\n{}\n{}\n{}",
            &HEAD_HASH[0..4],
            &REMOTE_HASH[0..4],
            &STASH_HASH[0..4],
            &TAG_HASH[0..4],
        )
        .into_bytes();

        assert!(result.is_ok());
        assert_eq!(output, expected);
    }

    #[test]
    fn show_hashes_with_limit_above_max() {
        let _pwd = create_temp_refs([
            Ref {
                dir: "tags",
                name: TAG_NAME,
                hash: TAG_HASH.as_bytes(),
            },
            Ref {
                dir: "remotes",
                name: REMOTE_NAME,
                hash: REMOTE_HASH.as_bytes(),
            },
        ]);

        let args = ShowRefArgs {
            head: false,
            heads: false,
            tags: false,
            hash: Some(50),
            abbrev: 40,
        };

        let mut output = Vec::new();
        let result = args.run(&mut output);
        let expected = format!(
            "{}\n{}\n{}\n{}",
            &HEAD_HASH, &REMOTE_HASH, &STASH_HASH, &TAG_HASH,
        )
        .into_bytes();

        assert!(result.is_ok());
        assert_eq!(output, expected);
    }

    #[test]
    fn allow_invalid_head_path_without_head_arg() {
        let pwd = create_temp_refs([]);
        let head_file = pwd.path().join(".git/HEAD");
        // Overwrite the HEAD file with an invalid path
        std::fs::write(&head_file, get_head_ref_content("invalid")).unwrap();

        let args = ShowRefArgs {
            head: false,
            heads: false,
            tags: false,
            hash: None,
            abbrev: 40,
        };

        let mut output = Vec::new();
        let result = args.run(&mut output);
        assert!(result.is_ok());
    }

    #[test]
    fn fail_on_invalid_head_path() {
        let pwd = create_temp_refs([]);
        let head_file = pwd.path().join(".git/HEAD");
        // Overwrite the HEAD file with an invalid path
        std::fs::write(&head_file, get_head_ref_content("invalid")).unwrap();

        let args = ShowRefArgs {
            head: true,
            heads: false,
            tags: false,
            hash: None,
            abbrev: 40,
        };

        let result = args.run(&mut Vec::new());
        assert!(result.is_err());
    }
}
