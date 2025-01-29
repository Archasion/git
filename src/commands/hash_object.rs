use crate::commands::CommandArgs;
use crate::utils::{format_header, git_object_dir, ObjectType};

use std::io::Write;
use std::path::PathBuf;

use anyhow::Context;
use clap::Parser;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use sha1::{Digest, Sha1};

impl CommandArgs for HashObjectArgs {
    /// Hashes the object and writes it to the `.git/objects` directory if requested.
    ///
    /// # Arguments
    ///
    /// * `self` - The `HashObjectArgs` containing the command arguments.
    ///
    /// # Returns
    ///
    /// * `anyhow::Result<()>` - The result of the command execution.
    fn run(self) -> anyhow::Result<()> {
        let HashObjectArgs {
            write,
            path,
            object_type,
        } = self;

        // Create blob from header and file content.
        let content = std::fs::read(&path).context(format!("read {}", path.display()))?;
        let header = format_header(object_type, content.len());
        let mut blob = header.into_bytes();
        blob.extend(content);

        // Hash blob with SHA-1.
        let hash = {
            let mut hasher = Sha1::new();
            hasher.update(&blob);
            format!("{:x}", hasher.finalize())
        };

        // Write blob to `.git/objects` directory if requested.
        if write {
            write_blob(&blob, &hash)?;
        }

        println!("{}", hash);
        Ok(())
    }
}

/// Writes the blob to the `.git/objects` directory.
///
/// # Arguments
///
/// * `blob` - The blob data to be written.
/// * `hash` - The hash of the blob.
///
/// # Returns
///
/// * `anyhow::Result<()>` - The result of the write operation.
fn write_blob(blob: &[u8], hash: &str) -> anyhow::Result<()> {
    // Create the object directory if it doesn't exist.
    let object_dir = git_object_dir(false)?.join(&hash[..2]);
    std::fs::create_dir_all(&object_dir).context("create subdir in .git/objects")?;

    // Compress the blob with zlib.
    let mut zlib = ZlibEncoder::new(Vec::new(), Compression::default());
    zlib.write_all(blob).context("write blob to zlib")?;
    let compressed = zlib.finish().context("finish zlib")?;

    // Write the compressed blob to the object file.
    let object_path = object_dir.join(&hash[2..]);
    std::fs::write(object_path, compressed).context("write compressed blob")
}

#[derive(Parser, Debug)]
pub(crate) struct HashObjectArgs {
    /// object type
    #[arg(short = 't', value_enum, default_value_t, name = "type")]
    object_type: ObjectType,
    /// write the object into the object database
    #[arg(short)]
    write: bool,
    /// process file as it were from this path
    #[arg(value_name = "file")]
    path: PathBuf,
}

#[cfg(test)]
mod tests {
    use super::{write_blob, HashObjectArgs};
    use crate::commands::CommandArgs;
    use crate::utils::test::TempPwd;
    use crate::utils::ObjectType;

    use std::fs;
    use std::path::PathBuf;

    use sha1::{Digest, Sha1};

    #[test]
    fn run_hashes_blob_and_prints_hash() {
        // Create a temporary file with some content.
        let temp_pwd = TempPwd::new();
        let file_path = temp_pwd.path().join("testfile.txt");
        fs::write(&file_path, b"test content").unwrap();

        let args = HashObjectArgs {
            write: false,
            path: file_path,
            object_type: ObjectType::Blob,
        };

        let result = args.run();
        assert!(result.is_ok());
    }

    #[test]
    fn run_writes_blob_to_git_objects() {
        // Create a temporary file with some content.
        let temp_pwd = TempPwd::new();
        let file_path = temp_pwd.path().join("testfile.txt");
        fs::write(&file_path, b"test content").unwrap();

        // Create the .git directory.
        fs::create_dir(temp_pwd.path().join(".git")).unwrap();

        let args = HashObjectArgs {
            write: true,
            path: file_path,
            object_type: ObjectType::Blob,
        };

        let result = args.run();
        assert!(result.is_ok());

        // Expected hash of the blob.
        let hash = {
            let mut hasher = Sha1::new();
            hasher.update(b"blob 12\0test content");
            format!("{:x}", hasher.finalize())
        };

        // Check that the object file was written to the `.git/objects` directory.
        let object_dir = temp_pwd.path().join(".git/objects").join(&hash[..2]);
        let object_path = object_dir.join(&hash[2..]);
        assert!(object_path.exists());
    }

    #[test]
    fn run_fails_on_nonexistent_file() {
        let args = HashObjectArgs {
            write: false,
            path: PathBuf::from("nonexistent.txt"),
            object_type: ObjectType::Blob,
        };

        let result = args.run();
        assert!(result.is_err());
    }

    #[test]
    fn write_blob_creates_object_directory() {
        // Create a temporary directory for testing.
        let temp_pwd = TempPwd::new();
        let blob = b"blob 12\0test content";

        // Create the .git directory.
        fs::create_dir(temp_pwd.path().join(".git")).unwrap();

        // Expected hash of the blob.
        let hash = {
            let mut hasher = Sha1::new();
            hasher.update(blob);
            format!("{:x}", hasher.finalize())
        };

        let result = write_blob(blob, &hash);
        assert!(result.is_ok());

        // Check that the object directory was created.
        let object_dir = temp_pwd.path().join(".git/objects").join(&hash[..2]);
        assert!(object_dir.exists());
    }

    #[test]
    fn write_blob_writes_compressed_blob() {
        // Create a temporary directory for testing.
        let temp_pwd = TempPwd::new();
        let blob = b"blob 12\0test content";

        // Create the .git directory.
        fs::create_dir(temp_pwd.path().join(".git")).unwrap();

        // Expected hash of the blob.
        let hash = {
            let mut hasher = Sha1::new();
            hasher.update(blob);
            format!("{:x}", hasher.finalize())
        };

        let result = write_blob(blob, &hash);
        assert!(result.is_ok());

        // Check that the object file was written to the `.git/objects` directory.
        let object_dir = temp_pwd.path().join(".git/objects").join(&hash[..2]);
        let object_path = object_dir.join(&hash[2..]);
        assert!(object_path.exists());
    }
}
