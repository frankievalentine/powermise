//! Atomic persistence primitives for powermise-generated state.
mod generated;
mod ledger;

pub use generated::{
    GENERATED_MANIFEST_SCHEMA_VERSION, GeneratedConfigArtifact, GeneratedFileChange,
    GeneratedFileChangeKind, GeneratedManifest, inspect_generated_file, validate_selection_name,
};
pub use ledger::{LEDGER_SCHEMA_VERSION, Ledger, OwnershipRecord, load_ledger, write_ledger};

use std::{
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    process,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use sha2::{Digest, Sha256};
use thiserror::Error;

static TEMPORARY_FILE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// The result of attempting to atomically persist state.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum AtomicWriteResult {
    /// The target did not exist and was created.
    Created,
    /// The target existed with different contents and was replaced.
    Updated,
    /// The target already contained the requested bytes and was not modified.
    Unchanged,
}

/// An error while validating or persisting state.
#[derive(Debug, Error)]
pub enum StateError {
    /// The supplied path cannot identify a file to write.
    #[error("invalid state path `{path}`: {reason}")]
    InvalidPath { path: PathBuf, reason: &'static str },
    /// An I/O operation failed for the indicated path.
    #[error("could not {operation} `{path}`: {source}")]
    Io {
        operation: &'static str,
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    /// A selection name is unsafe for use in generated file names.
    #[error(
        "invalid selection name `{selection}`; expected lowercase letters, numbers, and hyphens"
    )]
    InvalidSelectionName { selection: String },
    /// JSON state could not be serialized or parsed.
    #[error("could not {operation} JSON state `{path}`: {source}")]
    Json {
        operation: &'static str,
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    /// An ownership record was requested for an operation not owned by powermise.
    #[error("operation `{operation_id}` is owned by `{owner}`, not powermise")]
    UnownedOperation { operation_id: String, owner: String },
    /// A persisted state schema is newer or otherwise unsupported.
    #[error("unsupported {kind} schema version {version}; expected {expected}")]
    UnsupportedSchema {
        kind: &'static str,
        version: u32,
        expected: u32,
    },
}

/// Returns the lowercase SHA-256 hexadecimal digest for `contents`.
pub fn content_fingerprint(contents: &[u8]) -> String {
    format!("{:x}", Sha256::digest(contents))
}

/// Writes `contents` to `path` atomically unless it already contains those bytes.
///
/// Missing parent directories are created. Changed content is first written and
/// synchronized to a temporary file in the target directory, then renamed over
/// the target.
pub fn write_atomically(path: &Path, contents: &[u8]) -> Result<AtomicWriteResult, StateError> {
    validate_target_path(path)?;
    let resolved = resolve_symlink(path)?;
    let path = resolved.as_path();

    let existing_contents = match fs::read(path) {
        Ok(existing_contents) => Some(existing_contents),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(source) => {
            return Err(StateError::Io {
                operation: "read state file",
                path: path.to_path_buf(),
                source,
            });
        }
    };

    if existing_contents.as_deref() == Some(contents) {
        return Ok(AtomicWriteResult::Unchanged);
    }

    let parent = target_parent(path)?;
    fs::create_dir_all(parent).map_err(|source| StateError::Io {
        operation: "create state directory",
        path: parent.to_path_buf(),
        source,
    })?;

    let (temporary_path, mut temporary_file) = create_temporary_file(parent)?;
    let write_result = temporary_file
        .write_all(contents)
        .and_then(|()| temporary_file.sync_all())
        .and_then(|()| match fs::metadata(path) {
            // Keep the replaced file's permissions (an rc file's mode, an
            // executable bit) instead of the temporary file's defaults.
            Ok(metadata) => fs::set_permissions(&temporary_path, metadata.permissions()),
            Err(_) => Ok(()),
        });
    drop(temporary_file);

    if let Err(source) = write_result {
        remove_temporary_file(&temporary_path);
        return Err(StateError::Io {
            operation: "write temporary state file",
            path: temporary_path,
            source,
        });
    }

    if let Err(source) = fs::rename(&temporary_path, path) {
        remove_temporary_file(&temporary_path);
        return Err(StateError::Io {
            operation: "replace state file",
            path: path.to_path_buf(),
            source,
        });
    }

    Ok(if existing_contents.is_some() {
        AtomicWriteResult::Updated
    } else {
        AtomicWriteResult::Created
    })
}

/// Follows a symlinked target to the file it points at, so the rename replaces
/// that file and the link survives. Dotfile managers such as chezmoi and stow
/// commonly symlink files like `~/.zshrc`; renaming over the link would
/// silently detach it. A dangling link is an error rather than something to
/// overwrite.
fn resolve_symlink(path: &Path) -> Result<PathBuf, StateError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            fs::canonicalize(path).map_err(|source| StateError::Io {
                operation: "resolve symlinked state file",
                path: path.to_path_buf(),
                source,
            })
        }
        _ => Ok(path.to_path_buf()),
    }
}

fn validate_target_path(path: &Path) -> Result<(), StateError> {
    if path.as_os_str().is_empty() || path.file_name().is_none() {
        return Err(StateError::InvalidPath {
            path: path.to_path_buf(),
            reason: "a file name is required",
        });
    }

    Ok(())
}

fn target_parent(path: &Path) -> Result<&Path, StateError> {
    match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => Ok(parent),
        Some(_) => Ok(Path::new(".")),
        None => Err(StateError::InvalidPath {
            path: path.to_path_buf(),
            reason: "a parent directory is required",
        }),
    }
}

fn create_temporary_file(parent: &Path) -> Result<(PathBuf, File), StateError> {
    for _ in 0..32 {
        let temporary_path = parent.join(temporary_file_name());
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary_path)
        {
            Ok(file) => return Ok((temporary_path, file)),
            Err(source) if source.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(source) => {
                return Err(StateError::Io {
                    operation: "create temporary state file",
                    path: temporary_path,
                    source,
                });
            }
        }
    }

    Err(StateError::Io {
        operation: "create unique temporary state file",
        path: parent.to_path_buf(),
        source: io::Error::new(
            io::ErrorKind::AlreadyExists,
            "could not allocate a unique temporary state file name",
        ),
    })
}

fn temporary_file_name() -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let sequence = TEMPORARY_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);

    format!(".powermise-{}-{timestamp}-{sequence}.tmp", process::id(),)
}

fn remove_temporary_file(path: &Path) {
    let _ = fs::remove_file(path);
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
        process,
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::{AtomicWriteResult, StateError, content_fingerprint, write_atomically};

    static TEST_DIRECTORY_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    struct TestDirectory {
        path: PathBuf,
    }

    impl TestDirectory {
        fn new() -> Self {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            let sequence = TEST_DIRECTORY_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "powermise-state-test-{}-{timestamp}-{sequence}",
                process::id(),
            ));

            fs::create_dir_all(&path).expect("test directory should be created");
            Self { path }
        }

        fn file(&self, name: &str) -> PathBuf {
            self.path.join(name)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn first_write_creates_target_and_parent_directories() {
        let directory = TestDirectory::new();
        let target = directory.file("nested/state.toml");

        assert_eq!(
            write_atomically(&target, b"generated state").expect("write should succeed"),
            AtomicWriteResult::Created,
        );
        assert_eq!(
            fs::read(target).expect("target should exist"),
            b"generated state"
        );
    }

    #[test]
    fn replacement_completely_replaces_existing_content() {
        let directory = TestDirectory::new();
        let target = directory.file("state.toml");
        fs::write(&target, b"obsolete generated state").expect("initial state should be written");

        assert_eq!(
            write_atomically(&target, b"new").expect("replacement should succeed"),
            AtomicWriteResult::Updated,
        );
        assert_eq!(fs::read(target).expect("target should exist"), b"new");
    }

    #[test]
    fn identical_content_is_a_no_op() {
        let directory = TestDirectory::new();
        let target = directory.file("state.toml");
        fs::write(&target, b"unchanged").expect("initial state should be written");
        let modified_before = fs::metadata(&target)
            .expect("target metadata should exist")
            .modified()
            .expect("target modification time should be readable");

        assert_eq!(
            write_atomically(&target, b"unchanged").expect("no-op should succeed"),
            AtomicWriteResult::Unchanged,
        );
        assert_eq!(
            fs::metadata(&target)
                .expect("target metadata should exist")
                .modified()
                .expect("target modification time should be readable"),
            modified_before,
        );
    }

    #[test]
    fn fingerprint_is_deterministic_sha256_hex() {
        assert_eq!(
            content_fingerprint(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        );
        assert_eq!(
            content_fingerprint(b"powermise"),
            content_fingerprint(b"powermise"),
        );
        assert_ne!(
            content_fingerprint(b"powermise"),
            content_fingerprint(b"Powermise"),
            "fingerprints are byte-exact, including case"
        );
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_targets_keep_their_link_and_permissions() {
        use std::os::unix::fs::{PermissionsExt, symlink};

        let directory = TestDirectory::new();
        let real = directory.file("dotfiles/zshrc");
        fs::create_dir_all(real.parent().unwrap()).unwrap();
        fs::write(&real, b"user content\n").unwrap();
        fs::set_permissions(&real, fs::Permissions::from_mode(0o640)).unwrap();
        let link = directory.file(".zshrc");
        symlink(&real, &link).unwrap();

        assert_eq!(
            write_atomically(&link, b"user content\nmanaged\n").expect("write should succeed"),
            AtomicWriteResult::Updated,
        );
        assert!(
            fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink(),
            "the symlink must survive the write"
        );
        assert_eq!(fs::read(&real).unwrap(), b"user content\nmanaged\n");
        assert_eq!(
            fs::metadata(&real).unwrap().permissions().mode() & 0o777,
            0o640
        );
    }

    #[cfg(unix)]
    #[test]
    fn failed_write_preserves_existing_target() {
        use std::os::unix::fs::PermissionsExt;

        let directory = TestDirectory::new();
        let target = directory.file("state.toml");
        fs::write(&target, b"last known good state").expect("initial state should be written");

        let original_permissions = fs::metadata(&directory.path)
            .expect("directory metadata should exist")
            .permissions();
        let mut read_only_permissions = original_permissions.clone();
        read_only_permissions.set_mode(0o555);
        fs::set_permissions(&directory.path, read_only_permissions)
            .expect("directory should become read-only");

        let result = write_atomically(&target, b"replacement state");

        fs::set_permissions(&directory.path, original_permissions)
            .expect("directory permissions should be restored");

        assert!(matches!(result, Err(StateError::Io { .. })));
        assert_eq!(
            fs::read(target).expect("existing target should remain readable"),
            b"last known good state",
        );
    }

    #[test]
    fn invalid_path_is_reported_with_the_input_path() {
        let path = Path::new("");

        assert!(matches!(
            write_atomically(path, b"state"),
            Err(StateError::InvalidPath { path: error_path, .. }) if error_path == path,
        ));
    }
}
