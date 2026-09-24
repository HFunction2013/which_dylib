# which_dylib

[![Crates.io](https://img.shields.io/crates/v/which_dylib.svg)](https://crates.io/crates/which_dylib)
[![Docs](https://docs.rs/which_dylib/badge.svg)](https://docs.rs/which_dylib)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

A cross-platform Rust library for locating dynamic/shared library files (`lib*.so`, `lib*.dylib`, `*.dll`) on the filesystem.

## Features

- **Cross-platform** — Linux, macOS, and Windows support with compile-time OS detection
- **Fluent builder API** — chainable configuration methods
- **Custom search paths** — add, remove, or completely override search roots
- **Default search paths** — automatically includes system directories, executable directory, and environment variables
- **Custom filename patterns** — define your own prefix/suffix or full filename generation logic
- **Depth control** — limit search depth or search recursively without limits
- **Strict mode** — detect ambiguous matches when multiple candidates exist
- **Exclusion support** — exclude specific directories from the search

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
which_dylib = "0.1"
```

## Quick Start

```rust
use which_dylib::FindLibBuilder;

fn main() {
    // Basic usage — find libfoo.so / libfoo.dylib / foo.dll
    match FindLibBuilder::new().find_result("foo") {
        Ok(path) => println!("Found at: {}", path.display()),
        Err(e) => eprintln!("Not found: {:?}", e),
    }
}
```

## Examples

### Custom search paths

```rust
let path = FindLibBuilder::new()
    .add_path("/opt/myapp/lib")
    .add_path("/home/user/custom/libs")
    .find("mylib");
```

### Disable default paths

```rust
let path = FindLibBuilder::new()
    .no_defaults()
    .add_path("/only/this/dir")
    .find("mylib");
```

### Custom filename pattern

```rust
// Search for libfoo_custom.so.1
let path = FindLibBuilder::new()
    .set_prefix_suffix("lib", "_custom.so.1")
    .find("foo");

// Fully custom filename logic
let path = FindLibBuilder::new()
    .set_filename_fn(|name| {
        if name.starts_with("ssl") {
            format!("lib{}_openssl.so.3", name)
        } else {
            format!("lib{}.so.1", name)
        }
    })
    .find("ssl");
```

### Search depth control

```rust
// Only search the given directories (default)
let path = FindLibBuilder::new().depth(1).find("mylib");

// Search up to 3 levels deep
let path = FindLibBuilder::new().depth(3).find("mylib");

// Unlimited depth
let path = FindLibBuilder::new().depth(-1).find("mylib");
```

### Strict mode (detect ambiguity)

```rust
use which_dylib::{FindLibBuilder, FindError};

let result = FindLibBuilder::new()
    .strict()
    .find_result("mylib");

match result {
    Ok(path) => println!("Found: {}", path.display()),
    Err(FindError::Ambiguous(paths)) => {
        eprintln!("Multiple matches found:");
        for p in paths {
            eprintln!("  {}", p.display());
        }
    }
    Err(e) => eprintln!("Error: {:?}", e),
}
```

### Assume a different target OS

```rust
// Search for Windows DLLs on a Linux machine
let path = FindLibBuilder::new()
    .assume_os_str("windows")
    .add_path("/wine/drive_c/Windows/System32")
    .find("kernel32");
```

### Exclude directories

```rust
let path = FindLibBuilder::new()
    .add_path("/usr/lib")
    .remove_path("/usr/lib/debug")
    .find("mylib");
```

## How It Works

### Search Path Priority

1. **Custom paths** — added via `add_path()` / `add_paths()` (highest priority)
2. **Executable directory** — the directory containing the current executable
3. **Current dylib directory** — the directory containing the currently loaded library
4. **Environment variables** — `LD_LIBRARY_PATH` (Linux), `DYLD_LIBRARY_PATH` (macOS), `PATH` (Windows)
5. **System directories** — `/usr/lib`, `/usr/local/lib`, `/lib`, etc.

### Filename Generation

By default, the library name is converted to the platform-specific format:

| OS | Pattern | Example |
|----|---------|---------|
| Linux | `lib{name}.so` | `libfoo.so` |
| macOS | `lib{name}.dylib` | `libfoo.dylib` |
| Windows | `{name}.dll` | `foo.dll` |

You can override this with `set_filename_fn()` or `set_prefix_suffix()`.

## API Overview

### Types

- `FindLibBuilder` — fluent builder for search configuration
- `FindError` — error enum (`NotFound`, `Ambiguous`)
- `TargetOs` — target OS enum (`Linux`, `Macos`, `Windows`, `Unknown`)

### Builder Methods

| Method | Description |
|--------|-------------|
| `new()` | Create a new builder with defaults |
| `depth(n)` | Set max search depth (`-1` = unlimited) |
| `add_path(p)` | Add a custom search path |
| `add_paths(iter)` | Add multiple search paths |
| `remove_path(p)` | Remove a path from search and add to exclusions |
| `no_defaults()` | Disable all default search paths |
| `strict()` | Return all matches instead of first only |
| `assume_os(os)` | Assume a specific target OS |
| `assume_os_str(s)` | Assume target OS from string |
| `set_filename_fn(f)` | Set custom filename generation function |
| `set_prefix_suffix(prefix, suffix)` | Set custom filename prefix/suffix |

### Search Methods

| Method | Returns | Description |
|--------|---------|-------------|
| `find(name)` | `Option<PathBuf>` | Returns first match or `None` |
| `find_result(name)` | `Result<PathBuf, FindError>` | Returns detailed result with error info |

## Platform Support

| Platform | Supported | Notes |
|----------|-----------|-------|
| Linux | ✅ | Full support |
| macOS | ✅ | Full support |
| Windows | ✅ | Full support |
| Other (BSD, etc.) | ⚠️ | Falls back to Linux-style naming |

## License

Licensed under the LICENSE.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.