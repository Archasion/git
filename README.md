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

## Testing

Due to the nature of the project, tests must be run sequentially. To run the tests, use the
following command:

```sh
cargo test -- --test-threads=1
```