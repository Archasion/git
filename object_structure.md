# Object Structure

This document describes the structure of Git objects.

## General Object Structure

> [Reference](https://git-scm.com/book/en/v2/Git-Internals-Git-Objects) (Object Storage)

All objects are stored in the `.git/objects` directory (or `$GIT_DIR/$GIT_OBJECT_DIRECTORY`) and
have the following structure:

```
{type} {size}\0{content}
```

- `{type}` is the type of the object (blob, tree, commit, tag).
- `{size}` is the size of the content in bytes.
- `{content}` is the actual content of the object.

## Blob

> [Reference](https://git-scm.com/book/en/v2/Git-Internals-Git-Objects) (Object Storage)

A blob object is a file. Its content is just the file data.

## Tree

> [Reference](https://stackoverflow.com/a/37105125/19244184)

A tree object represents a directory. It contains a list of entries (no separator), each of which
can be either a blob or a tree object.

The format of each entry is as follows:

```
{mode} {filename}\0{hash}
```

- `{mode}` is the file mode (e.g., `100644` for a file, `040000` for a directory).
- `{filename}` is the name of the file or directory.
- `{hash}` is the SHA-1 hash of the object represented in binary form.

## Commit

> [Reference](https://stackoverflow.com/a/37438460/19244184)

A commit object represents a commit. It contains a reference to a tree object, a list of parent
commits, an author, a committer, and a commit message.

The content of a commit object is as follows:

```
tree {tree_hash}
{parents}
author {author_name} {{author_email}} {author_date_seconds} {author_date_offset}
committer {committer_name} {{committer_email}} {committer_date_seconds} {committer_date_offset}

{commit_message}
```

- `{tree_hash}` is the SHA-1 hash of the tree object.
- `{parents}` is a list of parent commit objects (if any) of the form:
    ```
    parent {parent_1_hash}
    parent {parent_2_hash}
    ...
    ```
- `{author_name}` is the name of the author.
- `{author_email}` is the email address of the author.
- `{author_date_seconds}` is the author date in seconds since the Unix epoch.
- `{author_date_offset}` is the author date offset from UTC.
- `{committer_name}` is the name of the committer.
- `{committer_email}` is the email address of the committer.
- `{committer_date_seconds}` is the committer date in seconds since the Unix epoch.
- `{committer_date_offset}` is the committer date offset from UTC.
- `{commit_message}` is the commit message.