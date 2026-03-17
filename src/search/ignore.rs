use std::path::Path;

pub(crate) struct PatchignoreMatcher {
    matcher: ::ignore::gitignore::Gitignore,
}

impl PatchignoreMatcher {
    pub(crate) fn from_scope(scope: &Path) -> Self {
        let patchignore = scope.join(".patchignore");
        let matcher = if patchignore.is_file() {
            ::ignore::gitignore::Gitignore::new(&patchignore).0
        } else {
            ::ignore::gitignore::Gitignore::empty()
        };

        Self { matcher }
    }

    pub(crate) fn is_ignored(&self, path: &Path, is_dir: bool) -> bool {
        self.matcher
            .matched_path_or_any_parents(path, is_dir)
            .is_ignore()
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::PatchignoreMatcher;

    #[test]
    fn matches_scope_root_patchignore_rules() {
        let scope = Path::new("tests/fixtures/patchignore");
        let matcher = PatchignoreMatcher::from_scope(scope);

        assert!(matcher.is_ignored(&scope.join("generated.gen.rs"), false));
        assert!(matcher.is_ignored(&scope.join("root-only.rs"), false));
        assert!(matcher.is_ignored(&scope.join("ignored-dir/ignored_api.rs"), false));
        assert!(!matcher.is_ignored(&scope.join("ignored-dir/reincluded.rs"), false));
        assert!(!matcher.is_ignored(&scope.join("nested/root-only.rs"), false));
    }
}
