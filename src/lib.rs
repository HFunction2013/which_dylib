use std::path::{Path, PathBuf};
use std::str::FromStr;
use walkdir::WalkDir;

/// Errors that can occur when searching for a library.
#[derive(Debug)]
pub enum FindError {
    /// Library was not found in any search path.
    NotFound(String),
    /// Multiple candidates were found (only returned in strict mode).
    Ambiguous(Vec<PathBuf>),
}

/// Supported operating systems enumeration.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TargetOs {
    Linux,
    Macos,
    Windows,
    Unknown,
}

impl FromStr for TargetOs {
    /// Parse from string (case-insensitive).
    ///
    /// # Examples
    ///
    /// ```
    /// use which_dylib::TargetOs;
    /// use std::str::FromStr;
    /// 
    /// assert_eq!(TargetOs::from_str("linux").unwrap(), TargetOs::Linux);
    /// assert_eq!(TargetOs::from_str("DARWIN").unwrap(), TargetOs::Macos);
    /// assert_eq!(TargetOs::from_str("Win").unwrap(), TargetOs::Windows);
    /// assert_eq!(TargetOs::from_str("freebsd").unwrap(), TargetOs::Unknown);
    /// ```
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.to_lowercase().as_str() {
            "linux" | "gnu" => Self::Linux,
            "macos" | "darwin" | "apple" => Self::Macos,
            "windows" | "win" | "msvc" => Self::Windows,
            _ => Self::Unknown,
        })
    }
}

impl TargetOs {
    /// Returns the current compilation target OS.
    ///
    /// Uses `#[cfg]` attributes to detect the platform at compile time.
    pub fn current() -> Self {
        #[cfg(target_os = "linux")]
        {
            Self::Linux
        }
        #[cfg(target_os = "macos")]
        {
            Self::Macos
        }
        #[cfg(target_os = "windows")]
        {
            Self::Windows
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            Self::Unknown
        }
    }
}

/// Builder for configuring and executing library file searches.
///
/// This struct provides a fluent API for:
/// - Customizing search paths
/// - Setting search depth limits
/// - Excluding specific directories
/// - Targeting different operating systems
/// - Customizing library filename patterns
///
/// # Examples
///
/// Basic usage:
/// ```no_run
/// use which_dylib::FindLibBuilder;
///
/// let path = FindLibBuilder::new()
///     .find("mylib")
///     .expect("Library not found");
/// ```
///
/// With custom filename pattern:
/// ```no_run
/// use which_dylib::FindLibBuilder;
///
/// let path = FindLibBuilder::new()
///     .set_prefix_suffix("lib", "_debug.so")
///     .depth(3)
///     .find("mylib")
///     .expect("Library not found");
/// ```
pub type FileNameFn = Box<dyn Fn(&str) -> String>;
pub struct FindLibBuilder {
    custom_roots: Vec<PathBuf>,
    excluded: Vec<PathBuf>,
    depth: i32,
    use_defaults: bool,
    first_only: bool,
    /// Assumed target OS; uses compile-time cfg if None.
    assume_os: Option<TargetOs>,
    /// Custom filename generation function; overrides default library_filename behavior.
    filename_fn: Option<FileNameFn>,
}

impl Default for FindLibBuilder {
    fn default() -> Self {
        Self {
            custom_roots: Vec::new(),
            excluded: Vec::new(),
            depth: 1,
            use_defaults: true,
            first_only: true,
            assume_os: None,
            filename_fn: None,
        }
    }
}

impl FindLibBuilder {
    /// Creates a new `FindLibBuilder` with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    // ── Configuration Methods ──

    /// Sets the maximum search depth.
    ///
    /// * `1` - only search the given directories (default)
    /// * Positive values - search up to N levels deep
    /// * Negative values - unlimited depth
    pub fn depth(mut self, n: i32) -> Self {
        self.depth = n;
        self
    }

    /// Adds a single custom search path.
    pub fn add_path<P: AsRef<Path>>(mut self, p: P) -> Self {
        self.custom_roots.push(p.as_ref().to_path_buf());
        self
    }

    /// Adds multiple custom search paths from an iterator.
    pub fn add_paths<I, P>(mut self, paths: I) -> Self
    where
        I: IntoIterator<Item = P>,
        P: AsRef<Path>,
    {
        for p in paths {
            self.custom_roots.push(p.as_ref().to_path_buf());
        }
        self
    }

    /// Removes a path from search roots and adds it to exclusion list.
    ///
    /// This ensures the path won't be searched even if it appears in default paths.
    pub fn remove_path<P: AsRef<Path>>(mut self, p: P) -> Self {
        let target = p.as_ref();
        self.custom_roots.retain(|x| x != target);
        self.excluded.push(target.to_path_buf());
        self
    }

    /// Disables default search paths.
    ///
    /// When called, only explicitly added paths will be searched.
    pub fn no_defaults(mut self) -> Self {
        self.use_defaults = false;
        self
    }

    /// Enables strict mode: returns all matches instead of just the first one.
    ///
    /// In this mode, `find_result` may return `FindError::Ambiguous` if multiple
    /// libraries are found.
    pub fn strict(mut self) -> Self {
        self.first_only = false;
        self
    }

    /// Assumes a specific target operating system for filename generation.
    ///
    /// Affects:
    /// - Library filename format (`libxxx.so` / `libxxx.dylib` / `xxx.dll`)
    /// - System default search paths
    /// - Environment variable names (`LD_LIBRARY_PATH` / `DYLD_LIBRARY_PATH` / `PATH`)
    ///
    /// Note: This does NOT affect the actual compilation target.
    pub fn assume_os(mut self, os: impl Into<TargetOs>) -> Self {
        self.assume_os = Some(os.into());
        self
    }

    /// Sets `assume_os` via string (convenient for command-line/config file input).
    ///
    /// See [`TargetOs::from_str`] for supported strings.
    pub fn assume_os_str(mut self, s: &str) -> Self {
        self.assume_os = Some(TargetOs::from_str(s).expect("Unknown Error"));
        self
    }

    /// Sets a custom filename generation function.
    ///
    /// This completely overrides the default `library_filename` logic based on OS.
    /// The closure receives the raw library name (e.g., "foo") and should return
    /// the full filename (e.g., "libfoo_custom.so.1").
    ///
    /// # Examples
    ///
    /// ```
    /// use which_dylib::FindLibBuilder;
    ///
    /// // Fully custom filename
    /// let builder = FindLibBuilder::new()
    ///     .set_filename_fn(|name| format!("my_{}_v2.so", name));
    ///
    /// // Conditional logic based on library name
    /// let builder = FindLibBuilder::new()
    ///     .set_filename_fn(|name| {
    ///         if name.starts_with("ssl") {
    ///             format!("lib{}_openssl.so.3", name)
    ///         } else {
    ///             format!("lib{}.so.1", name)
    ///         }
    ///     });
    /// ```
    pub fn set_filename_fn<F>(mut self, f: F) -> Self
    where
        F: Fn(&str) -> String + 'static,
    {
        self.filename_fn = Some(Box::new(f));
        self
    }

    /// Convenience method to set custom prefix and suffix for library filenames.
    ///
    /// Final filename format: `{prefix}{name}{suffix}`
    ///
    /// # Examples
    ///
    /// ```
    /// use which_dylib::FindLibBuilder;
    ///
    /// // Search for libfoo_custom.so.1
    /// let builder = FindLibBuilder::new()
    ///     .set_prefix_suffix("lib", "_custom.so.1");
    ///
    /// // Search for my_foo_debug.dll
    /// let builder = FindLibBuilder::new()
    ///     .set_prefix_suffix("my_", "_debug.dll");
    ///
    /// // Search for just "foo" (no prefix, no suffix)
    /// let builder = FindLibBuilder::new()
    ///     .set_prefix_suffix("", "");
    /// ```
    pub fn set_prefix_suffix(mut self, prefix: &str, suffix: &str) -> Self {
        let prefix = prefix.to_string();
        let suffix = suffix.to_string();
        self.filename_fn = Some(Box::new(move |name| format!("{}{}{}", prefix, name, suffix)));
        self
    }

    // ── Internal Helpers ──

    /// Returns the effective target OS (assumed or compile-time).
    fn effective_os(&self) -> TargetOs {
        self.assume_os.unwrap_or_else(TargetOs::current)
    }

    /// Generates the library filename based on configuration.
    ///
    /// Priority order:
    /// 1. Custom `filename_fn` (if set)
    /// 2. Default OS-based naming convention
    fn library_filename(&self, name: &str) -> String {
        // Use custom function if provided
        if let Some(ref f) = self.filename_fn {
            return f(name);
        }

        // Fall back to OS-based naming
        let os = self.effective_os();
        match os {
            TargetOs::Linux => format!("lib{}.so", name),
            TargetOs::Macos => format!("lib{}.dylib", name),
            TargetOs::Windows => format!("{}.dll", name),
            TargetOs::Unknown => {
                // Fallback to Linux-style naming for unknown OS
                format!("lib{}.so", name)
            }
        }
    }

    /// Returns system default search paths for the effective OS.
    fn sys_dirs(&self) -> Vec<PathBuf> {
        let os = self.effective_os();
        match os {
            TargetOs::Linux => vec![
                "/usr/lib".into(),
                "/usr/local/lib".into(),
                "/lib".into(),
                "/usr/lib/x86_64-linux-gnu".into(), // Common multiarch path
            ],
            TargetOs::Macos => vec![
                "/usr/lib".into(),
                "/usr/local/lib".into(),
                "/opt/homebrew/lib".into(), // Apple Silicon Homebrew
            ],
            TargetOs::Windows => {
                vec![PathBuf::from(r"C:\Windows\System32"), PathBuf::from(r"C:\Windows")]
            }
            TargetOs::Unknown => vec![],
        }
    }

    /// Returns environment variable names for the effective OS.
    fn env_var_names(&self) -> &'static [&'static str] {
        let os = self.effective_os();
        match os {
            TargetOs::Linux => &["LD_LIBRARY_PATH"],
            TargetOs::Macos => &["DYLD_LIBRARY_PATH", "DYLD_FALLBACK_LIBRARY_PATH"],
            TargetOs::Windows => &["PATH"],
            TargetOs::Unknown => &[],
        }
    }

    // ── Core Search Logic ──

    /// Collects all search root directories.
    ///
    /// Order of precedence:
    /// 1. Custom paths (highest priority)
    /// 2. Executable directory
    /// 3. Current dylib directory
    /// 4. Environment variables
    /// 5. System directories (lowest priority)
    fn collect_roots(&self) -> Vec<PathBuf> {
        let mut roots: Vec<PathBuf> = Vec::new();

        // 1. Custom paths (highest priority)
        roots.extend(self.custom_roots.clone());

        // 2. Built-in paths
        if self.use_defaults {
            // Executable directory
            if let Some(exe) = process_path::get_executable_path()
                && let Some(d) = exe.parent()
            {
                roots.push(d.to_path_buf());
            }
            // Current dylib directory (plugin self-location)
            if let Some(dylib) = process_path::get_dylib_path()
                && let Some(d) = dylib.parent()
            {
                roots.push(d.to_path_buf());
            }
            // Environment variables
            for var in self.env_var_names() {
                if let Ok(val) = std::env::var(var) {
                    roots.extend(std::env::split_paths(&val));
                }
            }
            // System directories
            roots.extend(self.sys_dirs());
        }

        // Deduplicate while preserving order
        let mut seen = std::collections::HashSet::new();
        roots.retain(|p| seen.insert(p.canonicalize().unwrap_or_else(|_| p.clone())));

        roots
    }

    /// Checks whether a path is excluded from search.
    fn is_excluded(&self, p: &Path) -> bool {
        let canonical = p.canonicalize().ok();
        self.excluded.iter().any(|exc| {
            let exc_canonical = exc.canonicalize().ok().unwrap_or_else(|| exc.clone());
            // Exact match or prefix match
            canonical.as_deref() == Some(&exc_canonical) || p.starts_with(exc)
        })
    }

    /// Searches for the library file in a single root directory.
    fn search_in_dir(&self, root: &Path, filename: &str) -> Vec<PathBuf> {
        let mut results = Vec::new();

        let max_depth = if self.depth < 0 { usize::MAX } else { self.depth as usize };

        for entry in WalkDir::new(root)
            .max_depth(max_depth)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            // eprintln!("[*] PATH: {}", &path.display());
            if path.is_file()
                && path.file_name().and_then(|n| n.to_str()) == Some(filename)
                && !self.is_excluded(path)
            {
                results.push(path.to_path_buf());
            }
        }

        results
    }

    // ── Public Search Interface ──

    /// Searches for a library and returns a `Result`.
    ///
    /// # Errors
    ///
    /// Returns `FindError::NotFound` if the library cannot be found.
    /// Returns `FindError::Ambiguous` if multiple candidates exist (strict mode only).
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use which_dylib::FindLibBuilder;
    ///
    /// match FindLibBuilder::new().find_result("mylib") {
    ///     Ok(path) => println!("Found at: {:?}", path),
    ///     Err(e) => eprintln!("Error: {:?}", e),
    /// }
    /// ```
    pub fn find_result(&self, name: &str) -> Result<PathBuf, FindError> {
        let filename = self.library_filename(name);
        let roots = self.collect_roots();

        let mut all_found: Vec<PathBuf> = Vec::new();

        for root in &roots {
            if self.is_excluded(root) {
                continue;
            }
            // eprintln!("[*] Searched in {}", &root.display());
            let found = self.search_in_dir(root, &filename);
            all_found.extend(found);

            if self.first_only && !all_found.is_empty() {
                return Ok(all_found.swap_remove(0));
            }
        }

        if all_found.is_empty() {
            Err(FindError::NotFound(format!(
                "library '{}' (resolved to '{}') not found in any search path",
                name, filename
            )))
        } else if all_found.len() == 1 {
            Ok(all_found.swap_remove(0))
        } else {
            Err(FindError::Ambiguous(all_found))
        }
    }

    /// Simplified search: returns `Option<PathBuf>`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use which_dylib::FindLibBuilder;
    ///
    /// if let Some(path) = FindLibBuilder::new().find("mylib") {
    ///     println!("Found at: {:?}", path);
    /// }
    /// ```
    pub fn find(&self, name: &str) -> Option<PathBuf> {
        self.find_result(name).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_custom_filename_fn() {
        let builder = FindLibBuilder::new().set_filename_fn(|name| format!("my_{}_v2.so", name));

        assert_eq!(builder.library_filename("test"), "my_test_v2.so");
    }

    #[test]
    fn test_set_prefix_suffix() {
        let builder = FindLibBuilder::new().set_prefix_suffix("lib", "_debug.so.1");

        assert_eq!(builder.library_filename("mylib"), "libmylib_debug.so.1");
    }

    #[test]
    fn test_empty_prefix_suffix() {
        let builder = FindLibBuilder::new().set_prefix_suffix("", "");

        assert_eq!(builder.library_filename("mylib"), "mylib");
    }

    #[test]
    fn test_custom_fn_overrides_os() {
        let builder = FindLibBuilder::new()
            .set_filename_fn(|_| "custom_name.ext".to_string())
            .assume_os_str("windows");

        // Custom function should take precedence over OS setting
        assert_eq!(builder.library_filename("anything"), "custom_name.ext");
    }

    #[test]
    fn test_target_os_from_str() {
        assert_eq!(TargetOs::from_str("linux").expect("Unknown Error"), TargetOs::Linux);
        assert_eq!(TargetOs::from_str("GNU").expect("Unknown Error"), TargetOs::Linux);
        assert_eq!(TargetOs::from_str("macos").expect("Unknown Error"), TargetOs::Macos);
        assert_eq!(TargetOs::from_str("DARWIN").expect("Unknown Error"), TargetOs::Macos);
        assert_eq!(TargetOs::from_str("windows").expect("Unknown Error"), TargetOs::Windows);
        assert_eq!(TargetOs::from_str("MSVC").expect("Unknown Error"), TargetOs::Windows);
        assert_eq!(TargetOs::from_str("unknown").expect("Unknown Error"), TargetOs::Unknown);
    }

    #[test]
    fn test_library_filename_defaults() {
        let linux = FindLibBuilder::new().assume_os(TargetOs::Linux);
        assert_eq!(linux.library_filename("foo"), "libfoo.so");

        let macos = FindLibBuilder::new().assume_os(TargetOs::Macos);
        assert_eq!(macos.library_filename("foo"), "libfoo.dylib");

        let windows = FindLibBuilder::new().assume_os(TargetOs::Windows);
        assert_eq!(windows.library_filename("foo"), "foo.dll");
    }
}
