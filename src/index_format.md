# Git Index File Format (v2)

The index file is located in `.git/index` (or `$GIT_DIR/index`) and is used to store the current
state of the working directory. It contains a sorted list of all files in the repository, along with
their metadata and object data.

## File Structure

### 1. Header

- **[4-byte]** Signature: `DIRC`
    - Stands for "directory cache".
- **[4-byte]** Version number: `0x0002`
    - Indicates version 2.
- **[4-byte]** Number of index entries.

---

### 2. Entry Format (Repeated for Each Index Entry)

#### 2.1 File Metadata

- **[8-byte]** File creation time:
    - **[4-byte]** Seconds since epoch.
    - **[4-byte]** Nanoseconds.
- **[8-byte]** File modification time:
    - **[4-byte]** Seconds since epoch.
    - **[4-byte]** Nanoseconds.
- **[4-byte]** User ID of the file owner.
- **[4-byte]** Group ID of the file owner.
- **[2-byte]** File size (in bytes).

#### 2.2 Object Data

- **[20-byte]** SHA-1 hash of the object.

#### 2.3 Flags

- **[2-byte]** Flags (bitwise representation):
    - **[1-bit]** Assume valid flag (set by `git update-index --assume-unchanged`).
    - **[1-bit]** Extended flag (unused in v2, always `0`).
    - **[2-bit]** Stage (during merge):
        - `0` = Normal.
        - `1` = Base.
        - `2` = Ours.
        - `3` = Theirs.
    - **[12-bit]** Name length (excluding padding).

#### 2.4 Extended Flags

- **[2-byte]** Extended flags (unused in v2).

#### 2.5 Path Name

- **[variable]** Path name (relative to the repository root).

---

### 3. Padding

- **[1-8 bytes]** Zero padding:
    - Ensures the index entry's byte count is divisible by 8.

---

### 4. Footer

- **[20-byte]** Checksum:
    - SHA-1 hash over the entire content of the index (excluding this checksum).

---

## Example Representation

| Section  | Size      | Description                                  |
|----------|-----------|----------------------------------------------|
| Header   | 12 bytes  | Signature, version, and entry count.         |
| Entry    | Variable  | File metadata, object data, flags, and path. |
| Padding  | 1-8 bytes | Ensures 8-byte alignment.                    |
| Checksum | 20 bytes  | SHA-1 checksum for validation.               |