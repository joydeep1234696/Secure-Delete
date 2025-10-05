# Secure-Delete
Tool for secure deletion simply and soley


\# Secure Delete

\### A minimal, cross-platform, Rust-based file \& folder shredder

`secure\_delete` is a compact CLI tool written in Rust for \*\*secure, irreversible deletion\*\* of files or entire directories.  

It overwrites data multiple times using configurable patterns (`zeros`, `ones`, or `random`), renames files to random names, and unlinks them — all with zero dependencies beyond the standard library and `rand`.


---


\## Features

\- \*\*Cross-platform:\*\* Works on Windows, macOS, and Linux.
\- \*\*Multi-pass overwrite:\*\* Configurable number of passes (default: 3).
\- \*\*Configurable patterns:\*\* Choose between `zeros`, `ones`, or `random` fills.
\- \*\*Recursive directory deletion:\*\* Securely wipes entire folders.
\- \*\*File rename before removal:\*\* Random filename substitution before unlink.
\- \*\*Permission handling:\*\* Clears read-only flags before overwrite.
\- \*\*No external dependencies:\*\* Only uses `rand` crate and Rust’s standard library.

---

[![Build and Package](https://github.com/whisprer-specops/Secure-Delete/actions/workflows/rust-release.yml/badge.svg)](https://github.com/whisprer-specops/Secure-Delete/actions/workflows/rust-release.yml)

---

\## Installation

\### Prerequisites
\- Rust toolchain (1.70+ recommended)
\- Cargo
\### Build

```bash
git clone https://github.com/whisprer/secure\_delete.git
cd secure\_delete
cargo build --release
The compiled binary will be available at:
target/release/secure_delete

Optional
To install system-wide:
cargo install --path .

Usage
Syntax
secure_delete <file-or-directory> [--passes N] [--pattern zeros|ones|random] [--confirm]

Examples
Securely delete a file:
secure_delete secret.txt --passes 5 --pattern random --confirm

Wipe an entire directory:
secure_delete ./sensitive_data --passes 3 --pattern random --confirm

Quick delete without confirmation:
secure_delete notes.tmp

Design Philosophy
secure_delete emphasizes:
Transparency: Plain-text progress output, no hidden actions.
Reliability: Each pass flushes data to disk (sync_all()).
Portability: Works identically on Linux, Windows, macOS.
Minimalism: Lightweight; no GUI, no heavy libraries, no telemetry.
It’s designed for developers, researchers, and security enthusiasts who prefer a trustworthy open-source erasure tool without the bloat.

Notes & Limitations
Data recovery impossibility: Once overwritten and unlinked, data is irretrievable by normal means — use with extreme caution.
Filesystem considerations: Some SSDs and journaling filesystems (e.g., Btrfs, APFS, NTFS) may still retain blocks due to wear-leveling or copy-on-write mechanics. Physical destruction is required for absolute sanitization.
No free-space wiping: This tool targets specified files/directories only.

Development
Project structure
secure_delete/
├─ Cargo.toml
└─ src/
   └─ main.rs

Build commands
cargo fmt
cargo clippy
cargo test
cargo build --release

License
Licensed under the Hybrid License
 (you can replace with your Hybrid License as desired).

Contributing
Pull requests and forks are welcome!
See CONTRIBUTING.md
 for style and patch guidelines.

Security
Please read SECURITY.md
 before reporting vulnerabilities.

Credits
Created by whisprer (wofl / husklfren)
Special thanks to G-Petey for code extraction, refactoring, and docs.

