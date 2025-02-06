This is a simple attempt at re-creating some of the functionality of the `git` command in Rust.

> Run `cargo run` to see the help message.

## Features

- `hash-object` - Compute the hash of an object and optionally write it to the object database.
    - `-w` flag to write the object to the object database.
    - `-t` flag to specify the type of the object (supported: `blob`).
    - `<file>` argument to specify the file to hash.
- `init` - Create an empty Git repository.
    - `--bare` flag to create a bare repository.
    - `-b` or `--initial-branch` flag to specify the initial branch.
    - `-q` or `--quiet` flag to suppress the output.
    - `<directory>` argument to specify the directory to initialize.
- `cat-file` - Provide content or type and size information for repository objects.
    - `-t` flag to show the type of the object.
    - `-s` flag to show the size of the object.
    - `-p` flag to show the content of the object (pretty-print)
    - `--allow-unknown-type` flag to allow unknown object types (to be used with `-t` or `-s`).
    - `<object>` argument to specify the object to show.
- `show-ref` - List references in a local repository.
    - `--head` flag to include the HEAD reference.
    - `--tags` flag to show only tags.
    - `--heads` flag to show only heads.
    - `--hash=<n>` flag to only show the reference hashes (`n` is the number of characters to show, 4-40).
    - `--abbrev=<n>` flag to abbreviate the hashes to `n` characters (4-40)

## Testing

Due to the nature of the project, tests must be run single-threaded. This is enforced by the `RUST_TEST_THREADS=1` environment variable in the [`.cargo/config.toml`](./.cargo/config.toml) file.

To run the tests, use the following command:

```sh
cargo test
```