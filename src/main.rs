// secure_delete - minimal secure file shredder (Rust)
// Usage:
//   cargo build --release
//   ./target/release/secure_delete <file> [--passes N] [--pattern zeros|ones|random] [--confirm]
// Example:
//   secure_delete secret.zip --passes 3 --pattern random --confirm
//
// Notes:
// - Overwrites file contents in chunks (8 MiB by default).
// - After overwriting passes, renames file to a random name in same directory, optionally attempts to clear readonly bit, then removes file.
// - Cross-platform behavior: uses only std + rand; tries to set writable permissions before unlinking.

use rand::{rngs::OsRng, RngCore};
use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const DEFAULT_PASSES: usize = 3;
const CHUNK_SIZE: usize = 8 * 1024 * 1024; // 8 MiB chunk writes

#[derive(Debug, Clone, Copy)]
enum Pattern {
    Zeros,
    Ones,
    Random,
}

impl Pattern {
    fn from_str(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "zeros" => Some(Pattern::Zeros),
            "ones" => Some(Pattern::Ones),
            "random" => Some(Pattern::Random),
            _ => None,
        }
    }
}

fn print_usage_and_exit(program: &str) -> ! {
    eprintln!("Usage: {} <file> [--passes N] [--pattern zeros|ones|random] [--confirm]", program);
    eprintln!("Example: {} secret.zip --passes 3 --pattern random --confirm", program);
    std::process::exit(1);
}

fn ask_confirm(prompt: &str) -> io::Result<bool> {
    use std::io::BufRead;
    print!("{} [y/N]: ", prompt);
    io::Write::flush(&mut io::stdout())?;
    let stdin = io::stdin();
    let mut line = String::new();
    stdin.lock().read_line(&mut line)?;
    let trimmed = line.trim().to_ascii_lowercase();
    Ok(trimmed == "y" || trimmed == "yes")
}

/// Attempts to make the file writable (clears read-only attributes if present).
fn ensure_writable(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = fs::metadata(path) {
            let mut perm = meta.permissions();
            // add owner write bit
            let cur = perm.mode();
            let new = cur | 0o200;
            perm.set_mode(new);
            let _ = fs::set_permissions(path, perm);
        }
    }

    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        // On Windows the readonly attribute is a separate file attribute.
        // Use std to set permissions writable; also attempt to clear readonly via `set_permissions`.
        if let Ok(meta) = fs::metadata(path) {
            let mut perm = meta.permissions();
            perm.set_readonly(false);
            let _ = fs::set_permissions(path, perm);
        }
    }
}

/// Overwrite the file at `path` with the specified pattern for `passes` times.
/// Uses chunked writes and syncs to disk after each pass.
/// Returns Ok(()) on success; io::Error on failure.
fn overwrite_file(path: &Path, passes: usize, pattern: Pattern) -> io::Result<()> {
    let metadata = fs::metadata(path)?;
    let file_size = metadata.len();
    if file_size == 0 {
        // nothing to do but still try to unlink later
        return Ok(());
    }

    // Pre-prepare a static chunk buffer for zeros/ones to avoid repeated allocations
    let zeros = vec![0u8; CHUNK_SIZE];
    let ones = vec![0xFFu8; CHUNK_SIZE];

    // For random we will generate into a buffer each time.
    let mut rng = OsRng;

    // We'll open the file for write access.
    let mut file = OpenOptions::new().write(true).open(path)?;

    // For progress reporting:
    let total_bytes = file_size.checked_mul(passes as u64).unwrap_or(u64::MAX);
    let mut bytes_written_total: u64 = 0;
    let t0 = Instant::now();

    for pass in 0..passes {
        // Seek to start
        file.seek(SeekFrom::Start(0))?;

        let mut remaining = file_size;
        while remaining > 0 {
            let to_write = std::cmp::min(remaining, CHUNK_SIZE as u64) as usize;
            let buf: &[u8];

            match pattern {
                Pattern::Zeros => {
                    buf = &zeros[..to_write];
                    file.write_all(buf)?;
                }
                Pattern::Ones => {
                    buf = &ones[..to_write];
                    file.write_all(buf)?;
                }
                Pattern::Random => {
                    // fill a local buffer with random bytes and write
                    let mut rb = vec![0u8; to_write];
                    rng.fill_bytes(&mut rb);
                    file.write_all(&rb)?;
                }
            }

            bytes_written_total = bytes_written_total.saturating_add(to_write as u64);
            remaining -= to_write as u64;
        }

        // Force writes to disk
        file.sync_all()?;

        // small pause so progress prints nicely on very fast SSDs
        std::thread::sleep(Duration::from_millis(50));

        let elapsed = t0.elapsed();
        // crude progress line
        eprint!(
            "\rPass {}/{} completed (elapsed: {:.1}s). Total bytes written: {}       ",
            pass + 1,
            passes,
            elapsed.as_secs_f64(),
            bytes_written_total
        );
    }

    // final newline after progress
    eprintln!();

    Ok(())
}

/// Generate a random filename of the given length in same directory.
/// Returns the new PathBuf (existing file not created).
fn random_filename_in_same_dir(orig: &Path, len: usize) -> PathBuf {
    let mut name = String::with_capacity(len);
    let mut rng = OsRng;
    const CHARS: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    for _ in 0..len {
        let idx = (rng.next_u32() as usize) % CHARS.len();
        name.push(CHARS[idx] as char);
    }

    // preserve extension if present (replace name but keep extension)
    let mut new = orig.to_path_buf();
    if let Some(ext) = orig.extension() {
        let mut file_name = name;
        file_name.push('.');
        file_name.push_str(&ext.to_string_lossy());
        new.set_file_name(file_name);
    } else {
        new.set_file_name(name);
    }
    new
}

fn rename_to_random_and_unlink(path: &Path) -> io::Result<()> {
    // Attempt to rename file to random filename (same dir) several times
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    // choose random name length relative to original name length
    let orig_name_len = path.file_name().and_then(|s| s.to_str()).map(|s| s.len()).unwrap_or(12);
    let mut attempts = 0usize;
    let max_attempts = 8;
    loop {
        let candidate = random_filename_in_same_dir(path, std::cmp::max(8, orig_name_len));
        let candidate_path = parent.join(candidate.file_name().unwrap());
        // Try to rename; if target exists, retry
        let res = fs::rename(path, &candidate_path);
        match res {
            Ok(_) => {
                // Set permissions to owner-write only (best-effort)
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let _ = fs::set_permissions(&candidate_path, fs::Permissions::from_mode(0o600));
                }
                #[cfg(windows)]
                {
                    let mut perm = fs::metadata(&candidate_path)?.permissions();
                    perm.set_readonly(false);
                    let _ = fs::set_permissions(&candidate_path, perm);
                }

                // Finally remove file
                return fs::remove_file(&candidate_path);
            }
            Err(e) => {
                attempts += 1;
                if attempts >= max_attempts {
                    return Err(e);
                }
                // small jitter and retry
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    }
}

fn process_path(path: &Path, passes: usize, pattern: Pattern, require_confirm: bool) -> io::Result<()> {
    if !path.exists() {
        return Err(io::Error::new(io::ErrorKind::NotFound, "file not found"));
    }
    if path.is_dir() {
        return Err(io::Error::new(io::ErrorKind::Other, "path is a directory; secure_delete handles files only"));
    }

    if require_confirm {
        let prompt = format!("Securely delete file '{}' ?", path.display());
        if !ask_confirm(&prompt)? {
            println!("Skipping {}", path.display());
            return Ok(());
        }
    }

    ensure_writable(path);

    println!("Starting secure delete of {} ({} passes, pattern: {:?})", path.display(), passes, pattern);
    overwrite_file(path, passes, pattern)?;
    // attempt rename & unlink
    match rename_to_random_and_unlink(path) {
        Ok(_) => {
            println!("Successfully removed {}", path.display());
            Ok(())
        }
        Err(e) => {
            eprintln!("Warning: overwrite succeeded but remove failed: {}", e);
            // final attempt: try remove directly
            if let Err(e2) = fs::remove_file(path) {
                return Err(e2);
            }
            Ok(())
        }
    }
}

fn parse_usize_arg(it: &mut impl Iterator<Item = String>) -> Option<usize> {
    it.next().and_then(|s| s.parse::<usize>().ok())
}

fn main() {
    let mut args: Vec<String> = env::args().collect();
    let program = args.get(0).map(|s| s.as_str()).unwrap_or("secure_delete").to_string();
    if args.len() < 2 {
        print_usage_and_exit(&program);
    }

    // Defaults
    let mut passes = DEFAULT_PASSES;
    let mut pattern = Pattern::Random;
    let mut require_confirm = false;

    // parse positional first arg as file path; then parse options
    // simple parser: first non-flag arg after program is file; supports single file only
    let mut iter = args.into_iter();
    let _ = iter.next(); // skip program

    let file_arg = match iter.next() {
        Some(f) => f,
        None => print_usage_and_exit(&program),
    };

    let mut it = iter.peekable();
    while let Some(tok) = it.next() {
        match tok.as_str() {
            "--passes" | "-p" => {
                if let Some(v) = it.next() {
                    if let Ok(n) = v.parse::<usize>() {
                        passes = n.max(1);
                    } else {
                        eprintln!("Invalid passes value: {}", v);
                        print_usage_and_exit(&program);
                    }
                } else {
                    print_usage_and_exit(&program);
                }
            }
            "--pattern" => {
                if let Some(v) = it.next() {
                    if let Some(p) = Pattern::from_str(&v) {
                        pattern = p;
                    } else {
                        eprintln!("Unknown pattern: {} (use zeros|ones|random)", v);
                        print_usage_and_exit(&program);
                    }
                } else {
                    print_usage_and_exit(&program);
                }
            }
            "--confirm" | "-c" => {
                require_confirm = true;
            }
            "--help" | "-h" => {
                print_usage_and_exit(&program);
            }
            other => {
                eprintln!("Unknown option: {}", other);
                print_usage_and_exit(&program);
            }
        }
    }

    let path = PathBuf::from(file_arg);

    match process_path(&path, passes, pattern, require_confirm) {
        Ok(_) => {}
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(2);
        }
    }
}
