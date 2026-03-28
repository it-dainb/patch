use std::path::{Path, PathBuf};

pub mod deps;
pub mod files;
pub mod map;
pub mod read;
pub mod search;
pub mod symbol;

pub fn resolve_scope(scope: &Path) -> PathBuf {
    let base_cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    resolve_scope_from(&base_cwd, scope)
}

pub fn resolve_scope_from(base_cwd: &Path, scope: &Path) -> PathBuf {
    let candidate = if scope.is_relative() {
        base_cwd.join(scope)
    } else {
        scope.to_path_buf()
    };

    candidate
        .canonicalize()
        .unwrap_or_else(|_| scope.to_path_buf())
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::resolve_scope_from;

    #[test]
    fn resolve_scope_from_existing_relative_path_canonicalizes_against_base_cwd() {
        let base_cwd = Path::new(env!("CARGO_MANIFEST_DIR"));
        let scope = Path::new("tests/fixtures/patchignore");

        let resolved = resolve_scope_from(base_cwd, scope);
        let expected = base_cwd.join(scope).canonicalize().unwrap();

        assert_eq!(resolved, expected);
    }

    #[test]
    fn resolve_scope_from_absolute_existing_path_canonicalizes() {
        let base_cwd = Path::new(env!("CARGO_MANIFEST_DIR"));
        let scope = base_cwd
            .join("tests/fixtures/patchignore")
            .canonicalize()
            .unwrap();

        let resolved = resolve_scope_from(base_cwd, &scope);
        let expected = scope.canonicalize().unwrap();

        assert_eq!(resolved, expected);
    }

    #[test]
    fn resolve_scope_from_missing_relative_path_preserves_raw_input() {
        let base_cwd = Path::new(env!("CARGO_MANIFEST_DIR"));
        let scope = PathBuf::from(
            "tests/fixtures/patchignore/__missing_relative_scope_for_resolve_scope_from_test",
        );

        assert!(!base_cwd.join(&scope).exists());

        let resolved = resolve_scope_from(base_cwd, &scope);

        assert_eq!(resolved, scope);
    }
}
