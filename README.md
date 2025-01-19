This is a simple attempt at re-creating some of the functionality of the `git` command in Rust.

> Run `cargo run` to see the help message.

## Features

- `hash-object` - Compute the hash of an object and optionally write it to the object database.
  - `-w` flag to write the object to the object database.
  - `-t` flag to specify the type of the object (supported: `blob`).
  - `<file>` argument to specify the file to hash.

## Testing

Due to the nature of the project, tests must be run sequentially. To run the tests, use the following command:

```sh
cargo test -- --test-threads=1
```