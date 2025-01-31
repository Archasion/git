use std::fs::File;
use std::io::{BufRead, BufReader, Read, Write};

use anyhow::Context;
use clap::Args;
use flate2::read::ZlibDecoder;

use crate::commands::CommandArgs;
use crate::utils::{binary_to_hex_bytes, get_object_path, parse_header, ObjectType};

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
        ObjectType::Blob => zlib.read_to_end(&mut buf)?,
        ObjectType::Tree => read_tree(&mut zlib, &mut buf)?,
        // Bail out if the object type is not supported
        t => anyhow::bail!("unsupported object type: {:?}", t),
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

fn read_tree(zlib: &mut BufReader<ZlibDecoder<File>>, buf: &mut Vec<u8>) -> anyhow::Result<usize> {
    let mut entry = Vec::new();
    let mut object_size = 0;

    loop {
        // Read the entry mode
        let mut mode = Vec::new();
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
        binary_to_hex_bytes(&mut hash);

        // Find the object type of the entry
        let hash_str = std::str::from_utf8(&hash).context("object hash is not valid utf-8")?;
        let mut object_type = Vec::new();
        read_object_type(hash_str, false, &mut object_type)?;

        // Append the remaining entry fields
        entry.extend(object_type);
        entry.push(b' ');
        entry.extend(hash);
        entry.push(b'\t');
        entry.extend(name);
        entry.push(b'\n');

        // Append the entry to the buffer
        // and clear the entry for the next iteration
        buf.extend_from_slice(&entry);
        entry.clear();
    }

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
    #[arg(long, requires = "meta")]
    allow_unknown_type: bool,
    /// the object to display
    #[arg(name = "object")]
    object_hash: String,
}

#[derive(Args, Debug)]
#[group(id = "flags", required = true)]
struct CatFileFlags {
    /// show object type
    #[arg(short = 't', group = "meta")]
    show_type: bool,
    /// show object size
    #[arg(short, group = "meta")]
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
    use crate::utils::env;
    use crate::utils::test::{TempEnv, TempPwd};

    const OBJECT_CONTENT: &str = "Hello, World!";
    const OBJECT_HASH: &str = "b45ef6fec89518d314f546fd6c3025367b721684";
    const OBJECT_HASH_UNKNOWN_TYPE: &str = "de7a5d7e25b0b0700efda74301e3afddf222f2da"; // type: unknown
    const OBJECT_HASH_INVALID_SIZE: &str = "5eacd92a2d45548f23ddee14fc6401a141f2dc9f"; // size: 0
    const OBJECT_TYPE: &str = "blob";

    /// Get the compressed representation of [`OBJECT_CONTENT`] and its header
    fn compress_object() -> Vec<u8> {
        let object = format!(
            "{} {}\0{}",
            OBJECT_TYPE,
            OBJECT_CONTENT.len(),
            OBJECT_CONTENT
        );
        let mut zlib = ZlibEncoder::new(Vec::new(), Compression::default());
        zlib.write_all(object.as_bytes()).unwrap();
        zlib.finish().unwrap()
    }

    /// Get the compressed representation of [`OBJECT_CONTENT`] with an unknown type in the header
    fn compress_object_unknown_type() -> Vec<u8> {
        let object = format!("unknown {}\0{}", OBJECT_CONTENT.len(), OBJECT_CONTENT);
        let mut zlib = ZlibEncoder::new(Vec::new(), Compression::default());
        zlib.write_all(object.as_bytes()).unwrap();
        zlib.finish().unwrap()
    }

    /// Get the compressed representation of [`OBJECT_CONTENT`] with an invalid size in the header
    fn compress_object_invalid_size() -> Vec<u8> {
        let object = format!("{} 0\0{}", OBJECT_TYPE, OBJECT_CONTENT);
        let mut zlib = ZlibEncoder::new(Vec::new(), Compression::default());
        zlib.write_all(object.as_bytes()).unwrap();
        zlib.finish().unwrap()
    }

    #[test]
    fn displays_object_content() {
        // Unset the GIT_DIR and GIT_OBJECT_DIRECTORY environment variables
        let _git_dir_env = TempEnv::new(env::GIT_DIR, None);
        let _git_object_dir_env = TempEnv::new(env::GIT_OBJECT_DIRECTORY, None);

        let temp_pwd = TempPwd::new();
        let object_path = format!(".git/objects/{}/{}", &OBJECT_HASH[..2], &OBJECT_HASH[2..]);
        let object_path = temp_pwd.path().join(object_path);

        // Create the object path and write the hashed content
        fs::create_dir_all(object_path.parent().unwrap()).unwrap();
        fs::write(&object_path, compress_object()).unwrap();

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
        assert_eq!(output, OBJECT_CONTENT.as_bytes());
    }

    #[test]
    fn exits_successfully() {
        // Unset the GIT_DIR and GIT_OBJECT_DIRECTORY environment variables
        let _git_dir_env = TempEnv::new(env::GIT_DIR, None);
        let _git_object_dir_env = TempEnv::new(env::GIT_OBJECT_DIRECTORY, None);

        let temp_pwd = TempPwd::new();
        let object_path = format!(".git/objects/{}/{}", &OBJECT_HASH[..2], &OBJECT_HASH[2..]);
        let object_path = temp_pwd.path().join(object_path);

        // Create the object path and write the hashed content
        fs::create_dir_all(object_path.parent().unwrap()).unwrap();
        fs::write(&object_path, compress_object()).unwrap();

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
        // Unset the GIT_DIR and GIT_OBJECT_DIRECTORY environment variables
        let _git_dir_env = TempEnv::new(env::GIT_DIR, None);
        let _git_object_dir_env = TempEnv::new(env::GIT_OBJECT_DIRECTORY, None);

        let temp_pwd = TempPwd::new();
        let object_path = format!(".git/objects/{}/{}", &OBJECT_HASH[..2], &OBJECT_HASH[2..]);
        let object_path = temp_pwd.path().join(object_path);

        // Create the object path and write the hashed content
        fs::create_dir_all(object_path.parent().unwrap()).unwrap();
        fs::write(&object_path, compress_object()).unwrap();

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
        assert_eq!(output, OBJECT_TYPE.as_bytes());
    }

    #[test]
    fn displays_object_size() {
        // Unset the GIT_DIR and GIT_OBJECT_DIRECTORY environment variables
        let _git_dir_env = TempEnv::new(env::GIT_DIR, None);
        let _git_object_dir_env = TempEnv::new(env::GIT_OBJECT_DIRECTORY, None);

        let temp_pwd = TempPwd::new();
        let object_path = format!(".git/objects/{}/{}", &OBJECT_HASH[..2], &OBJECT_HASH[2..]);
        let object_path = temp_pwd.path().join(object_path);

        // Create the object path and write the hashed content
        fs::create_dir_all(object_path.parent().unwrap()).unwrap();
        fs::write(&object_path, compress_object()).unwrap();

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
        assert_eq!(output, OBJECT_CONTENT.len().to_string().as_bytes());
    }

    #[test]
    fn displays_object_type_with_unknown_type() {
        // Unset the GIT_DIR and GIT_OBJECT_DIRECTORY environment variables
        let _git_dir_env = TempEnv::new(env::GIT_DIR, None);
        let _git_object_dir_env = TempEnv::new(env::GIT_OBJECT_DIRECTORY, None);

        let temp_pwd = TempPwd::new();
        let object_path = format!(
            ".git/objects/{}/{}",
            &OBJECT_HASH_UNKNOWN_TYPE[..2],
            &OBJECT_HASH_UNKNOWN_TYPE[2..]
        );
        let object_path = temp_pwd.path().join(object_path);

        // Create the object path and write the hashed content
        fs::create_dir_all(object_path.parent().unwrap()).unwrap();
        fs::write(&object_path, compress_object_unknown_type()).unwrap();

        let args = CatFileArgs {
            flags: CatFileFlags {
                show_type: true,
                size: false,
                exit_zero: false,
                pretty_print: false,
            },
            allow_unknown_type: true,
            object_hash: OBJECT_HASH_UNKNOWN_TYPE.to_string(),
        };

        let mut output = Vec::new();
        let result = args.run(&mut output);

        assert!(result.is_ok());
        assert_eq!(output, b"unknown");
    }

    #[test]
    fn displays_object_size_with_unknown_type() {
        // Unset the GIT_DIR and GIT_OBJECT_DIRECTORY environment variables
        let _git_dir_env = TempEnv::new(env::GIT_DIR, None);
        let _git_object_dir_env = TempEnv::new(env::GIT_OBJECT_DIRECTORY, None);

        let temp_pwd = TempPwd::new();
        let object_path = format!(
            ".git/objects/{}/{}",
            &OBJECT_HASH_UNKNOWN_TYPE[..2],
            &OBJECT_HASH_UNKNOWN_TYPE[2..]
        );
        let object_path = temp_pwd.path().join(object_path);

        // Create the object path and write the hashed content
        fs::create_dir_all(object_path.parent().unwrap()).unwrap();
        fs::write(&object_path, compress_object_unknown_type()).unwrap();

        let args = CatFileArgs {
            flags: CatFileFlags {
                show_type: false,
                size: true,
                exit_zero: false,
                pretty_print: false,
            },
            allow_unknown_type: true,
            object_hash: OBJECT_HASH_UNKNOWN_TYPE.to_string(),
        };

        let mut output = Vec::new();
        let result = args.run(&mut output);

        assert!(result.is_ok());
        assert_eq!(output, OBJECT_CONTENT.len().to_string().as_bytes());
    }

    #[test]
    fn fails_to_display_object_type_with_unknown_type() {
        // Unset the GIT_DIR and GIT_OBJECT_DIRECTORY environment variables
        let _git_dir_env = TempEnv::new(env::GIT_DIR, None);
        let _git_object_dir_env = TempEnv::new(env::GIT_OBJECT_DIRECTORY, None);

        let temp_pwd = TempPwd::new();
        let object_path = format!(
            ".git/objects/{}/{}",
            &OBJECT_HASH_UNKNOWN_TYPE[..2],
            &OBJECT_HASH_UNKNOWN_TYPE[2..]
        );
        let object_path = temp_pwd.path().join(object_path);

        // Create the object path and write the hashed content
        fs::create_dir_all(object_path.parent().unwrap()).unwrap();
        fs::write(&object_path, compress_object_unknown_type()).unwrap();

        let args = CatFileArgs {
            flags: CatFileFlags {
                show_type: true,
                size: false,
                exit_zero: false,
                pretty_print: false,
            },
            allow_unknown_type: false,
            object_hash: OBJECT_HASH_UNKNOWN_TYPE.to_string(),
        };

        let mut output = Vec::new();
        let result = args.run(&mut output);

        assert!(result.is_err());
    }

    #[test]
    fn fails_to_display_object_size_with_unknown_type() {
        // Unset the GIT_DIR and GIT_OBJECT_DIRECTORY environment variables
        let _git_dir_env = TempEnv::new(env::GIT_DIR, None);
        let _git_object_dir_env = TempEnv::new(env::GIT_OBJECT_DIRECTORY, None);

        let temp_pwd = TempPwd::new();
        let object_path = format!(
            ".git/objects/{}/{}",
            &OBJECT_HASH_UNKNOWN_TYPE[..2],
            &OBJECT_HASH_UNKNOWN_TYPE[2..]
        );
        let object_path = temp_pwd.path().join(object_path);

        // Create the object path and write the hashed content
        fs::create_dir_all(object_path.parent().unwrap()).unwrap();
        fs::write(&object_path, compress_object_unknown_type()).unwrap();

        let args = CatFileArgs {
            flags: CatFileFlags {
                show_type: false,
                size: true,
                exit_zero: false,
                pretty_print: false,
            },
            allow_unknown_type: false,
            object_hash: OBJECT_HASH_UNKNOWN_TYPE.to_string(),
        };

        let mut output = Vec::new();
        let result = args.run(&mut output);

        assert!(result.is_err());
    }

    #[test]
    fn fails_to_display_object_content_with_invalid_size() {
        // Unset the GIT_DIR and GIT_OBJECT_DIRECTORY environment variables
        let _git_dir_env = TempEnv::new(env::GIT_DIR, None);
        let _git_object_dir_env = TempEnv::new(env::GIT_OBJECT_DIRECTORY, None);

        let temp_pwd = TempPwd::new();
        let object_path = format!(
            ".git/objects/{}/{}",
            &OBJECT_HASH_INVALID_SIZE[..2],
            &OBJECT_HASH_INVALID_SIZE[2..]
        );
        let object_path = temp_pwd.path().join(object_path);

        // Create the object path and write the hashed content
        fs::create_dir_all(object_path.parent().unwrap()).unwrap();
        fs::write(&object_path, compress_object_invalid_size()).unwrap();

        let args = CatFileArgs {
            flags: CatFileFlags {
                show_type: false,
                size: false,
                exit_zero: false,
                pretty_print: true,
            },
            allow_unknown_type: false,
            object_hash: OBJECT_HASH_INVALID_SIZE.to_string(),
        };

        let mut output = Vec::new();
        let result = args.run(&mut output);

        assert!(result.is_err());
    }

    #[test]
    fn fails_to_display_object_content_with_unknown_type() {
        // Unset the GIT_DIR and GIT_OBJECT_DIRECTORY environment variables
        let _git_dir_env = TempEnv::new(env::GIT_DIR, None);
        let _git_object_dir_env = TempEnv::new(env::GIT_OBJECT_DIRECTORY, None);

        let temp_pwd = TempPwd::new();
        let object_path = format!(
            ".git/objects/{}/{}",
            &OBJECT_HASH_INVALID_SIZE[..2],
            &OBJECT_HASH_INVALID_SIZE[2..]
        );
        let object_path = temp_pwd.path().join(object_path);

        // Create the object path and write the hashed content
        fs::create_dir_all(object_path.parent().unwrap()).unwrap();
        fs::write(&object_path, compress_object_invalid_size()).unwrap();

        let args = CatFileArgs {
            flags: CatFileFlags {
                show_type: false,
                size: false,
                exit_zero: false,
                pretty_print: true,
            },
            allow_unknown_type: false,
            object_hash: OBJECT_HASH_INVALID_SIZE.to_string(),
        };

        let mut output = Vec::new();
        let result = args.run(&mut output);

        assert!(result.is_err());
    }

    #[test]
    fn displays_object_type_with_invalid_size() {
        // Unset the GIT_DIR and GIT_OBJECT_DIRECTORY environment variables
        let _git_dir_env = TempEnv::new(env::GIT_DIR, None);
        let _git_object_dir_env = TempEnv::new(env::GIT_OBJECT_DIRECTORY, None);

        let temp_pwd = TempPwd::new();
        let object_path = format!(
            ".git/objects/{}/{}",
            &OBJECT_HASH_INVALID_SIZE[..2],
            &OBJECT_HASH_INVALID_SIZE[2..]
        );
        let object_path = temp_pwd.path().join(object_path);

        // Create the object path and write the hashed content
        fs::create_dir_all(object_path.parent().unwrap()).unwrap();
        fs::write(&object_path, compress_object_invalid_size()).unwrap();

        let args = CatFileArgs {
            flags: CatFileFlags {
                show_type: true,
                size: false,
                exit_zero: false,
                pretty_print: false,
            },
            allow_unknown_type: false,
            object_hash: OBJECT_HASH_INVALID_SIZE.to_string(),
        };

        let mut output = Vec::new();
        let result = args.run(&mut output);

        assert!(result.is_ok());
        assert_eq!(output, OBJECT_TYPE.as_bytes());
    }

    #[test]
    fn displays_object_size_with_invalid_size() {
        // Unset the GIT_DIR and GIT_OBJECT_DIRECTORY environment variables
        let _git_dir_env = TempEnv::new(env::GIT_DIR, None);
        let _git_object_dir_env = TempEnv::new(env::GIT_OBJECT_DIRECTORY, None);

        let temp_pwd = TempPwd::new();
        let object_path = format!(
            ".git/objects/{}/{}",
            &OBJECT_HASH_INVALID_SIZE[..2],
            &OBJECT_HASH_INVALID_SIZE[2..]
        );
        let object_path = temp_pwd.path().join(object_path);

        // Create the object path and write the hashed content
        fs::create_dir_all(object_path.parent().unwrap()).unwrap();
        fs::write(&object_path, compress_object_invalid_size()).unwrap();

        let args = CatFileArgs {
            flags: CatFileFlags {
                show_type: false,
                size: true,
                exit_zero: false,
                pretty_print: false,
            },
            allow_unknown_type: false,
            object_hash: OBJECT_HASH_INVALID_SIZE.to_string(),
        };

        let mut output = Vec::new();
        let result = args.run(&mut output);

        assert!(result.is_ok());
        assert_eq!(output, b"0");
    }

    #[test]
    fn read_object_non_existent_hash() {
        // Unset the GIT_DIR and GIT_OBJECT_DIRECTORY environment variables
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
    fn read_header_non_existent_hash() {
        // Unset the GIT_DIR and GIT_OBJECT_DIRECTORY environment variables
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
