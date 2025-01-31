# Object Structure

This document describes the structure of Git objects.

## General Object Structure

All objects are stored in the `.git/objects` directory (or `$GIT_DIR/$GIT_OBJECT_DIRECTORY`) and
have the following structure:

```
<type> <size>\0<content>
```

## Blob

A blob object is a file. It contains the contents of the file.

```
blob <size>\0<content>
```

## Tree

A tree object is a directory. It contains a list of entries, each of which contains a mode, a
filename, and a hash of a tree or blob object. The entries are sorted by filename.

```
tree <size>\0<content>
```

The content of a tree is a list of entries, each of which contains a mode, a filename, and a hash of
a tree or blob object. The entries do not have a separator between them. Note that the SHA-1 hash is
binary, not hex.

The format of each entry is as follows:

```
<permission> <filename>\0<20-byte SHA-1 as binary>
```