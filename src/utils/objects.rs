//! Utilities for working with Git objects

use std::fmt;

use anyhow::Context;
use clap::ValueEnum;

/// Format the header of a `.git/objects` file
pub(crate) fn format_header<O, S>(object_type: O, size: S) -> String
where
    O: fmt::Display,
    S: fmt::Display,
{
    format!("{object_type} {size}\0")
}

/// Parse the header of a `.git/objects` file into the [`ObjectHeader`] struct.
pub(crate) fn parse_header(header: &[u8]) -> anyhow::Result<ObjectHeader> {
    // Split the header into type and size
    let mut header = header.splitn(2, |&b| b == b' ');

    let object_type = header.next().context("invalid object header")?;
    let size = header.next().context("invalid object header")?;
    let size = &size[..size.len().saturating_sub(1)]; // Remove the trailing null byte

    Ok(ObjectHeader { object_type, size })
}

/// The type of object in the Git object database
#[derive(Default, Debug, ValueEnum, Clone)]
pub(crate) enum ObjectType {
    #[default]
    Blob,
    Tree,
    Commit,
    Tag,
}

/// The header of a Git object
pub(crate) struct ObjectHeader<'a> {
    /// The type of object
    pub(crate) object_type: &'a [u8],
    /// The size of the object in bytes
    pub(crate) size: &'a [u8],
}

impl ObjectHeader<'_> {
    /// Parse the size of the object
    pub(crate) fn parse_size(&self) -> anyhow::Result<usize> {
        let size = std::str::from_utf8(self.size)
            .context("object size is not valid utf-8")?
            .parse::<usize>()
            .context("object size is not a number")?;

        Ok(size)
    }

    /// Parse the type of the object
    pub(crate) fn parse_type(&self) -> anyhow::Result<ObjectType> {
        ObjectType::try_from(self.object_type)
    }
}

impl fmt::Display for ObjectType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ObjectType::Blob => write!(f, "blob"),
            ObjectType::Tree => write!(f, "tree"),
            ObjectType::Commit => write!(f, "commit"),
            ObjectType::Tag => write!(f, "tag"),
        }
    }
}

impl TryFrom<&[u8]> for ObjectType {
    type Error = anyhow::Error;

    fn try_from(value: &[u8]) -> anyhow::Result<Self> {
        match value {
            b"blob" => Ok(ObjectType::Blob),
            b"tree" => Ok(ObjectType::Tree),
            b"commit" => Ok(ObjectType::Commit),
            b"tag" => Ok(ObjectType::Tag),
            _ => {
                let value = std::str::from_utf8(value).context("object type is not valid utf-8")?;
                anyhow::bail!("unknown object type: {}", value)
            },
        }
    }
}
