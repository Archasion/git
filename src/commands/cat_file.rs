use std::fs::File;
use std::io::{BufRead, BufReader, Read, Write};

use anyhow::Context;
use clap::Args;
use flate2::read::ZlibDecoder;

use crate::commands::CommandArgs;
use crate::utils::objects::{parse_header, ObjectType};
use crate::utils::{get_object_path, hex};

impl CommandArgs for CatFileArgs {
    fn run<W>(self, writer: &mut W) -> anyhow::Result<()>
    where
        W: Write,
    {
        if self.flags.show_type {
            return read_object_type(&self.object_hash, self.allow_unknown_type, writer);
        }
        if self.flags.size {
            return read_object_size(&self.object_hash, self.allow_unknown_type, writer);
        }
        if self.flags.exit_zero || self.flags.pretty_print {
            return read_object_pretty(&self.object_hash, self.flags.exit_zero, writer);
        }
        unreachable!("either -t, -s, -e, or -p must be specified");
    }
}

fn read_object_pretty<W>(hash: &str, exit: bool, writer: &mut W) -> anyhow::Result<()>
where
    W: Write,
{
    let object_path = get_object_path(hash, true)?;
    let file = File::open(object_path)?;

    // Create a zlib decoder to read the object header and content
    let zlib = ZlibDecoder::new(file);
    let mut zlib = BufReader::new(zlib);

    // Read the object header
    let mut header = Vec::new();
    zlib.read_until(0, &mut header)?;
    let header = parse_header(&header)?;

    // Read the object content
    let mut buf = Vec::new();
    let object_size = match header.parse_type()? {
        ObjectType::Tree => read_tree_pretty(&mut zlib, &mut buf)?,
        // Blobs, commits, and tags are pretty-printed as is
        _ => zlib.read_to_end(&mut buf)?,
    };

    // Ensure the object size matches the header
    if header.parse_size()? != object_size {
        anyhow::bail!("object size does not match header");
    }

    // Exit early if the object exists and passes validation
    if exit {
        return Ok(());
    }

    // Output the object content to stdout
    writer.write_all(&buf).context("write object to stdout")
}

fn read_tree_pretty(
    zlib: &mut BufReader<ZlibDecoder<File>>,
    buf: &mut Vec<u8>,
) -> anyhow::Result<usize> {
    let mut entries = Vec::new();
    let mut object_size = 0;

    loop {
        let mut entry = Vec::new();

        // Read the entry mode
        let mut mode = Vec::with_capacity(6);
        zlib.read_until(b' ', &mut mode)?;
        // Exit the loop if the mode is empty
        // This indicates the end of the tree
        if mode.is_empty() {
            break;
        }
        entry.extend(mode);

        // Read the entry name (file name)
        let mut name = Vec::new();
        zlib.read_until(0, &mut name)?;

        // Read the entry hash
        // Allocate enough space for a 40-byte hex hash
        let mut hash = Vec::with_capacity(40);
        zlib.take(20).read_to_end(&mut hash)?;

        // Add the entry size to the total size
        object_size += entry.len() + hash.len() + name.len();
        // Convert the binary hash to hex
        hex::encode_in_place(&mut hash);

        // Find the object type of the entry
        let hash_str = std::str::from_utf8(&hash).context("object hash is not valid utf-8")?;
        let mut object_type = Vec::new();
        read_object_type(hash_str, false, &mut object_type)?;

        // Append the remaining entry fields
        entry.extend(object_type);
        entry.push(b' ');
        entry.extend(hash);
        entry.push(b'\t');
        name.pop(); // Remove the trailing null byte
        entry.extend(name);

        // Append the entry to the list of entries
        entries.push(entry);
    }

    // Append the entries to the buffer
    // joined by a newline character
    buf.extend(entries.join(&b'\n'));
    Ok(object_size)
}

fn read_object_type<W>(hash: &str, allow_unknown_type: bool, writer: &mut W) -> anyhow::Result<()>
where
    W: Write,
{
    let object_path = get_object_path(hash, true)?;
    let file = File::open(object_path)?;

    // Create a zlib decoder to read the object header
    let zlib = ZlibDecoder::new(file);
    let mut zlib = BufReader::new(zlib);

    // Read the object header
    let mut buf = Vec::new();
    zlib.read_until(b' ', &mut buf)?;
    buf.pop(); // Remove the trailing space

    // Validate the object type
    if !allow_unknown_type {
        ObjectType::try_from(buf.as_slice())?;
    }

    writer
        .write_all(&buf)
        .context("write object type to writer")
}

fn read_object_size<W>(hash: &str, allow_unknown_type: bool, writer: &mut W) -> anyhow::Result<()>
where
    W: Write,
{
    let object_path = get_object_path(hash, true)?;
    let file = File::open(object_path)?;

    // Create a zlib decoder to read the object header
    let zlib = ZlibDecoder::new(file);
    let mut zlib = BufReader::new(zlib);

    // Read the object header
    let mut buf = Vec::new();
    zlib.read_until(0, &mut buf)?;
    let header = parse_header(&buf)?;

    if !allow_unknown_type {
        // Bail out if the object type fails to parse
        header.parse_type()?;
    }

    writer
        .write_all(header.size)
        .context("write object size to writer")
}

#[derive(Args, Debug)]
pub(crate) struct CatFileArgs {
    #[command(flatten)]
    flags: CatFileFlags,
    /// allow -s and -t to work with broken/corrupt objects
    #[arg(long, requires = "header")]
    allow_unknown_type: bool,
    /// the object to display
    #[arg(name = "object")]
    object_hash: String,
}

#[derive(Args, Debug)]
#[group(id = "flags", required = true)]
struct CatFileFlags {
    /// show object type
    #[arg(short = 't', group = "header")]
    show_type: bool,
    /// show object size
    #[arg(short, group = "header")]
    size: bool,
    /// check if <object> exists
    #[arg(short)]
    exit_zero: bool,
    /// pretty-print <object> content
    #[arg(short)]
    pretty_print: bool,
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Write;

    use flate2::write::ZlibEncoder;
    use flate2::Compression;

    use crate::commands::cat_file::{CatFileArgs, CatFileFlags};
    use crate::commands::CommandArgs;
    use crate::utils::test::{TempEnv, TempPwd};
    use crate::utils::{env, hex};

    const BLOB_CONTENT: &str = "Hello, World!";
    const OBJECT_HASH: &str = "2f22503f99671604495c84465f0113d002193369";
    const OBJECT_PATH: &str = ".git/objects/2f/22503f99671604495c84465f0113d002193369";

    /// Get the compressed representation of [`BLOB_CONTENT`] and its header
    ///
    /// # Arguments
    ///
    /// * `valid_type` - Whether the object type should be valid (`blob`)
    /// * `valid_size` - Whether the object size should be valid (size of the content)
    ///
    /// # Returns
    ///
    /// The compressed representation of the blob object and its header
    fn compress_blob(valid_type: bool, valid_size: bool) -> Vec<u8> {
        let object = format!(
            "{} {}\0{}",
            if valid_type { "blob" } else { "unknown" },
            if valid_size { BLOB_CONTENT.len() } else { 0 },
            BLOB_CONTENT
        );
        let mut zlib = ZlibEncoder::new(Vec::new(), Compression::default());
        zlib.write_all(object.as_bytes()).unwrap();
        zlib.finish().unwrap()
    }

    /// Get the compressed representation of a tree object and its header
    ///
    /// # Arguments
    ///
    /// * `object_hash` - The hash of the object to reference
    /// * `valid_type` - Whether the object type should be valid (`tree`)
    /// * `valid_size` - Whether the object size should be valid (size of the content)
    ///
    /// # Returns
    ///
    /// The compressed representation of the tree object and its header
    fn compress_tree(object_hash: &str, valid_type: bool, valid_size: bool) -> Vec<u8> {
        let content = tree_content(object_hash, false);
        let mut object = format!(
            "{} {}\0",
            if valid_type { "tree" } else { "unknown" },
            if valid_size { content.len() } else { 0 }
        )
        .into_bytes();
        object.extend(content);

        let mut zlib = ZlibEncoder::new(Vec::new(), Compression::default());
        zlib.write_all(&object).unwrap();
        zlib.finish().unwrap()
    }

    /// Get the content of a tree object
    ///
    /// # Arguments
    ///
    /// * `object_hash` - The hash of the object to reference
    /// * `pretty` - Whether the content should be pretty-printed
    ///
    /// # Returns
    ///
    /// The content of the tree object
    fn tree_content(object_hash: &str, pretty: bool) -> Vec<u8> {
        if pretty {
            format!("100644 blob {}\tfile.txt", object_hash).into_bytes()
        } else {
            let object_hash_binary =
                hex::decode(object_hash.as_bytes()).expect("failed to convert hex to binary");
            let mut content = b"100644 file.txt\0".to_vec();
            content.extend(object_hash_binary);
            content
        }
    }

    #[test]
    fn displays_non_tree() {
        // Unset environmental variables to avoid conflicts
        let _git_dir_env = TempEnv::new(env::GIT_DIR, None);
        let _git_object_dir_env = TempEnv::new(env::GIT_OBJECT_DIRECTORY, None);

        let temp_pwd = TempPwd::new();
        let object_path = temp_pwd.path().join(OBJECT_PATH);

        // Create the object path and write the hashed content
        fs::create_dir_all(object_path.parent().unwrap()).unwrap();
        fs::write(&object_path, compress_blob(true, true)).unwrap();

        let args = CatFileArgs {
            flags: CatFileFlags {
                show_type: false,
                size: false,
                exit_zero: false,
                pretty_print: true,
            },
            allow_unknown_type: false,
            object_hash: OBJECT_HASH.to_string(),
        };

        let mut output = Vec::new();
        let result = args.run(&mut output);

        assert!(result.is_ok());
        assert_eq!(output, BLOB_CONTENT.as_bytes());
    }

    #[test]
    fn displays_tree() {
        // Unset environmental variables to avoid conflicts
        let _git_dir_env = TempEnv::new(env::GIT_DIR, None);
        let _git_object_dir_env = TempEnv::new(env::GIT_OBJECT_DIRECTORY, None);

        let temp_pwd = TempPwd::new();
        let tree_path = temp_pwd.path().join(OBJECT_PATH);
        let blob_hash_hex = "01c6a63b7fc32f6f49988a9a12b8d7d199febeab";

        // Create the object path and write the hashed content
        fs::create_dir_all(tree_path.parent().unwrap()).unwrap();
        fs::write(&tree_path, compress_tree(blob_hash_hex, true, true)).unwrap();

        let blob_path = temp_pwd
            .path()
            .join(".git/objects")
            .join(&blob_hash_hex[..2])
            .join(&blob_hash_hex[2..]);

        // Create the object path and write the hashed content
        fs::create_dir(blob_path.parent().unwrap()).unwrap();
        fs::write(&blob_path, compress_blob(true, true)).unwrap();

        let args = CatFileArgs {
            flags: CatFileFlags {
                show_type: false,
                size: false,
                exit_zero: false,
                pretty_print: true,
            },
            allow_unknown_type: false,
            object_hash: OBJECT_HASH.to_string(),
        };

        let mut output = Vec::new();
        let result = args.run(&mut output);

        assert!(result.is_ok());
        assert_eq!(output, tree_content(blob_hash_hex, true));
    }

    #[test]
    fn exits_successfully() {
        // Unset environmental variables to avoid conflicts
        let _git_dir_env = TempEnv::new(env::GIT_DIR, None);
        let _git_object_dir_env = TempEnv::new(env::GIT_OBJECT_DIRECTORY, None);

        let temp_pwd = TempPwd::new();
        let object_path = temp_pwd.path().join(OBJECT_PATH);

        // Create the object path and write the hashed content
        fs::create_dir_all(object_path.parent().unwrap()).unwrap();
        fs::write(&object_path, compress_blob(true, true)).unwrap();

        let args = CatFileArgs {
            flags: CatFileFlags {
                show_type: false,
                size: false,
                exit_zero: true,
                pretty_print: false,
            },
            allow_unknown_type: false,
            object_hash: OBJECT_HASH.to_string(),
        };

        let mut output = Vec::new();
        let result = args.run(&mut output);

        assert!(result.is_ok());
        assert!(output.is_empty());
    }

    #[test]
    fn displays_object_type() {
        // Unset environmental variables to avoid conflicts
        let _git_dir_env = TempEnv::new(env::GIT_DIR, None);
        let _git_object_dir_env = TempEnv::new(env::GIT_OBJECT_DIRECTORY, None);

        let temp_pwd = TempPwd::new();
        let object_path = temp_pwd.path().join(OBJECT_PATH);

        // Create the object path and write the hashed content
        fs::create_dir_all(object_path.parent().unwrap()).unwrap();
        fs::write(&object_path, compress_blob(true, true)).unwrap();

        let args = CatFileArgs {
            flags: CatFileFlags {
                show_type: true,
                size: false,
                exit_zero: false,
                pretty_print: false,
            },
            allow_unknown_type: false,
            object_hash: OBJECT_HASH.to_string(),
        };

        let mut output = Vec::new();
        let result = args.run(&mut output);

        assert!(result.is_ok());
        assert_eq!(output, b"blob");
    }

    #[test]
    fn displays_object_size() {
        // Unset environmental variables to avoid conflicts
        let _git_dir_env = TempEnv::new(env::GIT_DIR, None);
        let _git_object_dir_env = TempEnv::new(env::GIT_OBJECT_DIRECTORY, None);

        let temp_pwd = TempPwd::new();
        let object_path = temp_pwd.path().join(OBJECT_PATH);

        // Create the object path and write the hashed content
        fs::create_dir_all(object_path.parent().unwrap()).unwrap();
        fs::write(&object_path, compress_blob(true, true)).unwrap();

        let args = CatFileArgs {
            flags: CatFileFlags {
                show_type: false,
                size: true,
                exit_zero: false,
                pretty_print: false,
            },
            allow_unknown_type: false,
            object_hash: OBJECT_HASH.to_string(),
        };

        let mut output = Vec::new();
        let result = args.run(&mut output);

        assert!(result.is_ok());
        assert_eq!(output, BLOB_CONTENT.len().to_string().as_bytes());
    }

    #[test]
    fn displays_object_type_with_unknown_type() {
        // Unset environmental variables to avoid conflicts
        let _git_dir_env = TempEnv::new(env::GIT_DIR, None);
        let _git_object_dir_env = TempEnv::new(env::GIT_OBJECT_DIRECTORY, None);

        let temp_pwd = TempPwd::new();
        let object_path = temp_pwd.path().join(OBJECT_PATH);

        // Create the object path and write the hashed content
        fs::create_dir_all(object_path.parent().unwrap()).unwrap();
        fs::write(&object_path, compress_blob(false, true)).unwrap();

        let args = CatFileArgs {
            flags: CatFileFlags {
                show_type: true,
                size: false,
                exit_zero: false,
                pretty_print: false,
            },
            allow_unknown_type: true,
            object_hash: OBJECT_HASH.to_string(),
        };

        let mut output = Vec::new();
        let result = args.run(&mut output);

        assert!(result.is_ok());
        assert_eq!(output, b"unknown");
    }

    #[test]
    fn displays_object_size_with_unknown_type() {
        // Unset environmental variables to avoid conflicts
        let _git_dir_env = TempEnv::new(env::GIT_DIR, None);
        let _git_object_dir_env = TempEnv::new(env::GIT_OBJECT_DIRECTORY, None);

        let temp_pwd = TempPwd::new();
        let object_path = temp_pwd.path().join(OBJECT_PATH);

        // Create the object path and write the hashed content
        fs::create_dir_all(object_path.parent().unwrap()).unwrap();
        fs::write(&object_path, compress_blob(false, true)).unwrap();

        let args = CatFileArgs {
            flags: CatFileFlags {
                show_type: false,
                size: true,
                exit_zero: false,
                pretty_print: false,
            },
            allow_unknown_type: true,
            object_hash: OBJECT_HASH.to_string(),
        };

        let mut output = Vec::new();
        let result = args.run(&mut output);

        assert!(result.is_ok());
        assert_eq!(output, BLOB_CONTENT.len().to_string().as_bytes());
    }

    #[test]
    fn fails_to_display_object_type_with_unknown_type() {
        // Unset environmental variables to avoid conflicts
        let _git_dir_env = TempEnv::new(env::GIT_DIR, None);
        let _git_object_dir_env = TempEnv::new(env::GIT_OBJECT_DIRECTORY, None);

        let temp_pwd = TempPwd::new();
        let object_path = temp_pwd.path().join(OBJECT_PATH);

        // Create the object path and write the hashed content
        fs::create_dir_all(object_path.parent().unwrap()).unwrap();
        fs::write(&object_path, compress_blob(false, true)).unwrap();

        let args = CatFileArgs {
            flags: CatFileFlags {
                show_type: true,
                size: false,
                exit_zero: false,
                pretty_print: false,
            },
            allow_unknown_type: false,
            object_hash: OBJECT_HASH.to_string(),
        };

        let result = args.run(&mut Vec::new());
        assert!(result.is_err());
    }

    #[test]
    fn fails_to_display_object_size_with_unknown_type() {
        // Unset environmental variables to avoid conflicts
        let _git_dir_env = TempEnv::new(env::GIT_DIR, None);
        let _git_object_dir_env = TempEnv::new(env::GIT_OBJECT_DIRECTORY, None);

        let temp_pwd = TempPwd::new();
        let object_path = temp_pwd.path().join(OBJECT_PATH);

        // Create the object path and write the hashed content
        fs::create_dir_all(object_path.parent().unwrap()).unwrap();
        fs::write(&object_path, compress_blob(false, true)).unwrap();

        let args = CatFileArgs {
            flags: CatFileFlags {
                show_type: false,
                size: true,
                exit_zero: false,
                pretty_print: false,
            },
            allow_unknown_type: false,
            object_hash: OBJECT_HASH.to_string(),
        };

        let result = args.run(&mut Vec::new());
        assert!(result.is_err());
    }

    #[test]
    fn fails_to_display_non_tree_with_invalid_size() {
        // Unset environmental variables to avoid conflicts
        let _git_dir_env = TempEnv::new(env::GIT_DIR, None);
        let _git_object_dir_env = TempEnv::new(env::GIT_OBJECT_DIRECTORY, None);

        let temp_pwd = TempPwd::new();
        let object_path = temp_pwd.path().join(OBJECT_PATH);

        // Create the object path and write the hashed content
        fs::create_dir_all(object_path.parent().unwrap()).unwrap();
        fs::write(&object_path, compress_blob(true, false)).unwrap();

        let args = CatFileArgs {
            flags: CatFileFlags {
                show_type: false,
                size: false,
                exit_zero: false,
                pretty_print: true,
            },
            allow_unknown_type: false,
            object_hash: OBJECT_HASH.to_string(),
        };

        let result = args.run(&mut Vec::new());
        assert!(result.is_err());
    }

    #[test]
    fn fails_to_display_tree_with_invalid_size() {
        // Unset environmental variables to avoid conflicts
        let _git_dir_env = TempEnv::new(env::GIT_DIR, None);
        let _git_object_dir_env = TempEnv::new(env::GIT_OBJECT_DIRECTORY, None);

        let temp_pwd = TempPwd::new();
        let tree_path = temp_pwd.path().join(OBJECT_PATH);
        let blob_hash_hex = "01c6a63b7fc32f6f49988a9a12b8d7d199febeab";

        // Create the object path and write the hashed content
        fs::create_dir_all(tree_path.parent().unwrap()).unwrap();
        fs::write(&tree_path, compress_tree(blob_hash_hex, true, false)).unwrap();

        let blob_path = temp_pwd
            .path()
            .join(".git/objects")
            .join(&blob_hash_hex[..2])
            .join(&blob_hash_hex[2..]);

        // Create the object path and write the hashed content
        fs::create_dir(blob_path.parent().unwrap()).unwrap();
        fs::write(&blob_path, compress_blob(true, true)).unwrap();

        let args = CatFileArgs {
            flags: CatFileFlags {
                show_type: false,
                size: false,
                exit_zero: false,
                pretty_print: true,
            },
            allow_unknown_type: false,
            object_hash: OBJECT_HASH.to_string(),
        };

        let result = args.run(&mut Vec::new());
        assert!(result.is_err());
    }

    #[test]
    fn fails_to_display_non_tree_with_unknown_type() {
        // Unset environmental variables to avoid conflicts
        let _git_dir_env = TempEnv::new(env::GIT_DIR, None);
        let _git_object_dir_env = TempEnv::new(env::GIT_OBJECT_DIRECTORY, None);

        let temp_pwd = TempPwd::new();
        let object_path = temp_pwd.path().join(OBJECT_PATH);

        // Create the object path and write the hashed content
        fs::create_dir_all(object_path.parent().unwrap()).unwrap();
        fs::write(&object_path, compress_blob(true, false)).unwrap();

        let args = CatFileArgs {
            flags: CatFileFlags {
                show_type: false,
                size: false,
                exit_zero: false,
                pretty_print: true,
            },
            allow_unknown_type: false,
            object_hash: OBJECT_HASH.to_string(),
        };

        let result = args.run(&mut Vec::new());
        assert!(result.is_err());
    }

    #[test]
    fn fails_to_display_tree_with_unknown_type() {
        // Unset environmental variables to avoid conflicts
        let _git_dir_env = TempEnv::new(env::GIT_DIR, None);
        let _git_object_dir_env = TempEnv::new(env::GIT_OBJECT_DIRECTORY, None);

        let temp_pwd = TempPwd::new();
        let tree_path = temp_pwd.path().join(OBJECT_PATH);
        let blob_hash_hex = "01c6a63b7fc32f6f49988a9a12b8d7d199febeab";

        // Create the object path and write the hashed content
        fs::create_dir_all(tree_path.parent().unwrap()).unwrap();
        fs::write(&tree_path, compress_tree(blob_hash_hex, false, true)).unwrap();

        let blob_path = temp_pwd
            .path()
            .join(".git/objects")
            .join(&blob_hash_hex[..2])
            .join(&blob_hash_hex[2..]);

        // Create the object path and write the hashed content
        fs::create_dir(blob_path.parent().unwrap()).unwrap();
        fs::write(&blob_path, compress_blob(true, true)).unwrap();

        let args = CatFileArgs {
            flags: CatFileFlags {
                show_type: false,
                size: false,
                exit_zero: false,
                pretty_print: true,
            },
            allow_unknown_type: false,
            object_hash: OBJECT_HASH.to_string(),
        };

        let result = args.run(&mut Vec::new());
        assert!(result.is_err());
    }

    #[test]
    fn displays_object_size_with_invalid_size() {
        // Unset environmental variables to avoid conflicts
        let _git_dir_env = TempEnv::new(env::GIT_DIR, None);
        let _git_object_dir_env = TempEnv::new(env::GIT_OBJECT_DIRECTORY, None);

        let temp_pwd = TempPwd::new();
        let object_path = temp_pwd.path().join(OBJECT_PATH);

        // Create the object path and write the hashed content
        fs::create_dir_all(object_path.parent().unwrap()).unwrap();
        fs::write(&object_path, compress_blob(true, false)).unwrap();

        let args = CatFileArgs {
            flags: CatFileFlags {
                show_type: false,
                size: true,
                exit_zero: false,
                pretty_print: false,
            },
            allow_unknown_type: false,
            object_hash: OBJECT_HASH.to_string(),
        };

        let mut output = Vec::new();
        let result = args.run(&mut output);

        assert!(result.is_ok());
        assert_eq!(output, b"0");
    }

    #[test]
    fn fails_to_display_object_with_invalid_hash() {
        // Unset environmental variables to avoid conflicts
        let _git_dir_env = TempEnv::new(env::GIT_DIR, None);
        let _git_object_dir_env = TempEnv::new(env::GIT_OBJECT_DIRECTORY, None);
        let _temp_pwd = TempPwd::new();

        let args = CatFileArgs {
            flags: CatFileFlags {
                show_type: false,
                size: false,
                exit_zero: false,
                pretty_print: true,
            },
            allow_unknown_type: false,
            object_hash: OBJECT_HASH.to_string(),
        };

        let result = args.run(&mut Vec::new());
        assert!(result.is_err());
    }

    #[test]
    fn fails_to_display_header_with_invalid_hash() {
        // Unset environmental variables to avoid conflicts
        let _git_dir_env = TempEnv::new(env::GIT_DIR, None);
        let _git_object_dir_env = TempEnv::new(env::GIT_OBJECT_DIRECTORY, None);
        let _temp_pwd = TempPwd::new();

        let args = CatFileArgs {
            flags: CatFileFlags {
                show_type: false,
                size: true,
                exit_zero: false,
                pretty_print: false,
            },
            allow_unknown_type: false,
            object_hash: OBJECT_HASH.to_string(),
        };

        let result = args.run(&mut Vec::new());
        assert!(result.is_err());
    }
}
