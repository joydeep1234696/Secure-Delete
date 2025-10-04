// secure_delete - minimal secure file shredder (Rust)
// Now supports recursive directory deletion.
// Usage:
//   secure_delete <file-or-directory> [--passes N] [--pattern zeros|ones|random] [--confirm]

use rand::{rngs::OsRng, RngCore};
use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const DEFAULT_PASSES: usize = 3;
const CHUNK_SIZE: usize = 8 * 1024 * 1024; // 8 MiB

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
    eprintln!("Usage: {} <file-or-dir> [--passes N] [--pattern zeros|ones|random] [--confirm]", program);
    std::process::exit(1);
}

fn ask_confirm(prompt: &str) -> io::Result<bool> {
    use std::io::BufRead;
    print!("{} [y/N]: ", prompt);
    io::Write::flush(&mut io::stdout())?;
    let stdin = io::stdin();
    let mut line = String::new();
    stdin.lock().read_line(&mut line)?;
    Ok(matches!(line.trim().to_ascii_lowercase().as_str(), "y" | "yes"))
}

fn ensure_writable(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = fs::metadata(path) {
            let mut perm = meta.permissions();
            let new = perm.mode() | 0o200;
            perm.set_mode(new);
            let _ = fs::set_permissions(path, perm);
        }
    }

    #[cfg(windows)]
    {
        if let Ok(meta) = fs::metadata(path) {
            let mut perm = meta.permissions();
            perm.set_readonly(false);
            let _ = fs::set_permissions(path, perm);
        }
    }
}

fn overwrite_file(path: &Path, passes: usize, pattern: Pattern) -> io::Result<()> {
    let metadata = fs::metadata(path)?;
    let file_size = metadata.len();
    if file_size == 0 {
        return Ok(());
    }

    let zeros = vec![0u8; CHUNK_SIZE];
    let ones = vec![0xFFu8; CHUNK_SIZE];
    let mut rng = OsRng;
    let mut file = OpenOptions::new().write(true).open(path)?;

    let total_bytes = file_size.checked_mul(passes as u64).unwrap_or(u64::MAX);
    let mut bytes_written_total: u64 = 0;
    let t0 = Instant::now();

    for pass in 0..passes {
        file.seek(SeekFrom::Start(0))?;
        let mut remaining = file_size;

        while remaining > 0 {
            let to_write = std::cmp::min(remaining, CHUNK_SIZE as u64) as usize;
            match pattern {
                Pattern::Zeros => file.write_all(&zeros[..to_write])?,
                Pattern::Ones => file.write_all(&ones[..to_write])?,
                Pattern::Random => {
                    let mut rb = vec![0u8; to_write];
                    rng.fill_bytes(&mut rb);
                    file.write_all(&rb)?;
                }
            }
            bytes_written_total += to_write as u64;
            remaining -= to_write as u64;
        }

        file.sync_all()?;
        std::thread::sleep(Duration::from_millis(50));

        eprint!(
            "\rPass {}/{} completed (elapsed: {:.1}s). Total bytes written: {}       ",
            pass + 1,
            passes,
            t0.elapsed().as_secs_f64(),
            bytes_written_total
        );
    }
    eprintln!();
    Ok(())
}

fn random_filename_in_same_dir(orig: &Path, len: usize) -> PathBuf {
    let mut name = String::with_capacity(len);
    let mut rng = OsRng;
    const CHARS: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    for _ in 0..len {
        name.push(CHARS[(rng.next_u32() as usize) % CHARS.len()] as char);
    }
    let mut new = orig.to_path_buf();
    if let Some(ext) = orig.extension() {
        let mut fname = name;
        fname.push('.');
        fname.push_str(&ext.to_string_lossy());
        new.set_file_name(fname);
    } else {
        new.set_file_name(name);
    }
    new
}

fn rename_to_random_and_unlink(path: &Path) -> io::Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let orig_len = path.file_name().and_then(|s| s.to_str()).map(|s| s.len()).unwrap_or(12);
    for _ in 0..8 {
        let candidate = random_filename_in_same_dir(path, orig_len.max(8));
        let candidate_path = parent.join(candidate.file_name().unwrap());
        if fs::rename(path, &candidate_path).is_ok() {
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
            return fs::remove_file(&candidate_path);
        }
    }
    // fallback: delete directly
    fs::remove_file(path)
}

// --- NEW: recursive handler for directories ---
fn process_directory(path: &Path, passes: usize, pattern: Pattern, confirm: bool) -> io::Result<()> {
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let entry_path = entry.path();
        if entry_path.is_dir() {
            process_directory(&entry_path, passes, pattern, confirm)?;
        } else {
            process_file(&entry_path, passes, pattern, confirm)?;
        }
    }

    // remove directory itself after cleaning
    fs::remove_dir(path)?;
    Ok(())
}

// --- existing file handler (slightly extracted for reuse) ---
fn process_file(path: &Path, passes: usize, pattern: Pattern, require_confirm: bool) -> io::Result<()> {
    if !path.exists() {
        return Err(io::Error::new(io::ErrorKind::NotFound, "file not found"));
    }
    if path.is_dir() {
        return Err(io::Error::new(io::ErrorKind::Other, "expected file but got directory"));
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
    match rename_to_random_and_unlink(path) {
        Ok(_) => {
            println!("Removed {}", path.display());
            Ok(())
        }
        Err(e) => {
            eprintln!("Warning: overwrite succeeded but remove failed: {}", e);
            if let Err(e2) = fs::remove_file(path) {
                return Err(e2);
            }
            Ok(())
        }
    }
}

// --- updated dispatcher ---
fn process_path(path: &Path, passes: usize, pattern: Pattern, require_confirm: bool) -> io::Result<()> {
    if !path.exists() {
        return Err(io::Error::new(io::ErrorKind::NotFound, "path not found"));
    }

    if path.is_dir() {
        if require_confirm {
            let prompt = format!("Recursively and securely delete directory '{}' ?", path.display());
            if !ask_confirm(&prompt)? {
                println!("Skipping {}", path.display());
                return Ok(());
            }
        }
        process_directory(path, passes, pattern, require_confirm)
    } else {
        process_file(path, passes, pattern, require_confirm)
    }
}

fn main() {
    let mut args: Vec<String> = env::args().collect();
    let program = args.get(0).map(|s| s.as_str()).unwrap_or("secure_delete").to_string();
    if args.len() < 2 {
        print_usage_and_exit(&program);
    }

    let mut passes = DEFAULT_PASSES;
    let mut pattern = Pattern::Random;
    let mut require_confirm = false;

    let mut iter = args.into_iter();
    let _ = iter.next();
    let file_arg = match iter.next() {
        Some(f) => f,
        None => print_usage_and_exit(&program),
    };

    let mut it = iter.peekable();
    while let Some(tok) = it.next() {
        match tok.as_str() {
            "--passes" | "-p" => {
                if let Some(v) = it.next() {
                    passes = v.parse::<usize>().unwrap_or(DEFAULT_PASSES).max(1);
                }
            }
            "--pattern" => {
                if let Some(v) = it.next() {
                    if let Some(p) = Pattern::from_str(&v) {
                        pattern = p;
                    } else {
                        eprintln!("Unknown pattern: {}", v);
                        print_usage_and_exit(&program);
                    }
                }
            }
            "--confirm" | "-c" => require_confirm = true,
            "--help" | "-h" => print_usage_and_exit(&program),
            other => {
                eprintln!("Unknown option: {}", other);
                print_usage_and_exit(&program);
            }
        }
    }

    let path = PathBuf::from(file_arg);
    if let Err(e) = process_path(&path, passes, pattern, require_confirm) {
        eprintln!("Error: {}", e);
        std::process::exit(2);
    }
}
