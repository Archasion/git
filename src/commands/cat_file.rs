use crate::commands::hash_object::ObjectType;
use crate::commands::{get_object_path, CommandArgs};

use std::fs::File;
use std::io::{BufRead, BufReader, Read, Write};

use anyhow::Context;
use clap::Args;
use flate2::read::ZlibDecoder;

impl CommandArgs for CatFileArgs {
    fn run(self) -> anyhow::Result<()> {
        let object_path = get_object_path(&self.object)?;
        let object = File::open(object_path)?;
        if self.object_type || self.size {
            return read_metadata(&self, &object);
        }
        if self.exit_zero || self.pretty_print {
            return read_content(&self, &object);
        }
        unreachable!("one of -t, -s, -e, or -p must be specified");
    }
}

fn read_content(args: &CatFileArgs, file: &File) -> anyhow::Result<()> {
    let zlib = ZlibDecoder::new(file);
    let mut zlib = BufReader::new(zlib);
    let mut header = Vec::new();
    zlib.read_until(0, &mut header)?;

    let header = std::str::from_utf8(&header).context("object header is not valid utf-8")?;
    let (object_type, size) = header
        .split_once(' ')
        .context("object header is not valid")?;
    let object_type = ObjectType::try_from(object_type.as_bytes())?;
    let size = size
        .trim_end_matches('\0')
        .parse::<usize>()
        .context("object size is not a valid integer")?;

    let mut content = Vec::new();
    zlib.read_to_end(&mut content)?;

    if size != content.len() {
        anyhow::bail!("object size does not match header");
    }

    if args.exit_zero {
        return Ok(());
    }

    if args.pretty_print {
        match object_type {
            ObjectType::Blob => {
                std::io::stdout()
                    .write_all(&content)
                    .context("write object to stdout")?;
            }
            _ => unimplemented!("pretty-printing for object type {:?}", object_type),
        }
    }

    Ok(())
}

fn read_metadata(args: &CatFileArgs, file: &File) -> anyhow::Result<()> {
    let zlib = ZlibDecoder::new(file);
    let mut zlib = BufReader::new(zlib);
    let mut object_type = Vec::new();

    // The object type is the first word in the object header
    zlib.read_until(b' ', &mut object_type)?;
    object_type.pop(); // Remove the trailing space

    if !args.allow_unknown_type {
        // Bail out if the object type fails to parse
        ObjectType::try_from(object_type.as_slice())?;
    }

    // If the object type is requested, print it and return
    if args.object_type {
        std::io::stdout()
            .write_all(&object_type)
            .context("write object type to stdout")?;
        return Ok(());
    }

    // If the object size is requested, print it and return
    if args.size {
        let mut size = Vec::new();
        // Read until the null byte to get the object size
        zlib.read_until(0, &mut size)?;
        std::io::stdout()
            .write_all(&size)
            .context("write object size to stdout")?;
        return Ok(());
    }

    unreachable!("either -t or -s must be specified");
}

#[derive(Args, Debug)]
pub(crate) struct CatFileArgs {
    /// show object type
    #[arg(short = 't', groups = ["meta", "flags"])]
    object_type: bool,
    /// show object size
    #[arg(short, groups = ["meta", "flags"])]
    size: bool,
    /// check if <object> exists
    #[arg(short, group = "flags")]
    exit_zero: bool,
    /// pretty-print <object> content
    #[arg(short, group = "flags")]
    pretty_print: bool,
    /// allow -s and -t to work with broken/corrupt objects
    #[arg(long, requires = "meta")]
    allow_unknown_type: bool,
    /// the object to display
    #[arg(name = "object")]
    object: String,
}
