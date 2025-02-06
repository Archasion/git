use std::io::Write;
use std::path::PathBuf;

use anyhow::Context;
use clap::Parser;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use sha1::{Digest, Sha1};

use crate::commands::CommandArgs;
use crate::utils::git_object_dir;
use crate::utils::objects::{format_header, ObjectType};

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
    fn run<W>(self, writer: &mut W) -> anyhow::Result<()>
    where
        W: Write,
    {
        // Create blob from header and file content.
        let content = std::fs::read(&self.path).context(format!("read {}", self.path.display()))?;
        let header = format_header(self.object_type, content.len());
        let mut blob = header.into_bytes();
        blob.extend(content);

        // Hash blob with SHA-1.
        // This is used to identify the blob in the object database.
        let hash = {
            let mut hasher = Sha1::new();
            hasher.update(&blob);
            // Format the hash as a hex string.
            format!("{:x}", hasher.finalize())
        };

        // Write blob to the object database if requested.
        if self.write {
            write_blob(&blob, &hash)?;
        }

        // Display the hash of the blob.
        writer.write_all(hash.as_bytes())?;
        Ok(())
    }
}

/// Writes the blob to the object database.
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
    // Split the hash into directory and file name.
    let (dir_name, file_name) = hash.split_at(2);

    // Create the object directory if it doesn't exist.
    let object_dir = git_object_dir(false)?.join(dir_name);
    std::fs::create_dir_all(&object_dir).context("create subdir in .git/objects")?;

    // Compress the blob with zlib.
    let mut zlib = ZlibEncoder::new(Vec::new(), Compression::default());
    zlib.write_all(blob).context("write blob to zlib")?;
    let compressed = zlib.finish().context("finish zlib")?;

    // Write the compressed blob to the object file.
    let object_path = object_dir.join(file_name);
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
    use std::fs;
    use std::path::PathBuf;

    use super::{write_blob, HashObjectArgs};
    use crate::commands::CommandArgs;
    use crate::utils::env;
    use crate::utils::objects::ObjectType;
    use crate::utils::test::{TempEnv, TempPwd};

    const OBJECT_CONTENT: &str = "Hello, World!";
    const FILE_NAME: &str = "testfile.txt";
    const OBJECT_HASH: &str = "b45ef6fec89518d314f546fd6c3025367b721684";

    #[test]
    fn hashes_blob_and_displays_hash() {
        let _env = TempEnv::from([(env::GIT_DIR, None), (env::GIT_OBJECT_DIRECTORY, None)]);

        let pwd = TempPwd::new();
        let file_path = pwd.path().join(FILE_NAME);
        fs::write(&file_path, OBJECT_CONTENT).unwrap();

        let args = HashObjectArgs {
            write: false,
            path: file_path,
            object_type: ObjectType::Blob,
        };

        let mut output = Vec::new();
        let result = args.run(&mut output);

        assert!(result.is_ok());
        assert_eq!(output, OBJECT_HASH.as_bytes());
    }

    #[test]
    fn writes_blob_to_object_database() {
        let _env = TempEnv::from([(env::GIT_DIR, None), (env::GIT_OBJECT_DIRECTORY, None)]);

        let pwd = TempPwd::new();
        let file_path = pwd.path().join(FILE_NAME);

        fs::write(&file_path, OBJECT_CONTENT).unwrap();
        // Create the .git directory.
        fs::create_dir_all(pwd.path().join(".git/objects")).unwrap();

        let args = HashObjectArgs {
            write: true,
            path: file_path,
            object_type: ObjectType::Blob,
        };

        let result = args.run(&mut Vec::new());
        assert!(result.is_ok());

        // Check that the object file was written to the object database.
        let (dir_name, file_name) = OBJECT_HASH.split_at(2);
        let object_path = pwd
            .path()
            .join(".git/objects")
            .join(dir_name)
            .join(file_name);
        assert!(object_path.exists());
    }

    #[test]
    fn fails_on_nonexistent_file() {
        let _env = TempEnv::from([(env::GIT_DIR, None), (env::GIT_OBJECT_DIRECTORY, None)]);
        let _pwd = TempPwd::new();

        let args = HashObjectArgs {
            write: false,
            path: PathBuf::from("nonexistent.txt"),
            object_type: ObjectType::Blob,
        };

        let result = args.run(&mut Vec::new());
        assert!(result.is_err());
    }

    #[test]
    fn write_blob_creates_object_database() {
        let _env = TempEnv::from([(env::GIT_DIR, None), (env::GIT_OBJECT_DIRECTORY, None)]);

        let pwd = TempPwd::new();
        let blob = format!("blob {}\0{}", OBJECT_CONTENT.len(), OBJECT_CONTENT);
        // Create the .git directory.
        fs::create_dir(pwd.path().join(".git")).unwrap();

        let result = write_blob(blob.as_bytes(), OBJECT_HASH);
        assert!(result.is_ok());

        // Check that the object directory and file were created.
        let (dir_name, file_name) = OBJECT_HASH.split_at(2);
        let object_dir = pwd
            .path()
            .join(".git/objects")
            .join(dir_name)
            .join(file_name);
        assert!(object_dir.exists());
    }
}
