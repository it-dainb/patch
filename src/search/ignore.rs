use std::path::Path;

pub(crate) struct DrailignoreMatcher {
    matcher: ::ignore::gitignore::Gitignore,
}

impl DrailignoreMatcher {
    pub(crate) fn from_scope(scope: &Path) -> Self {
        let drailignore = scope.join(".drailignore");
        let matcher = if drailignore.is_file() {
            ::ignore::gitignore::Gitignore::new(&drailignore).0
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

    use super::DrailignoreMatcher;

    #[test]
    fn matches_scope_root_drailignore_rules() {
        let scope = Path::new("tests/fixtures/drailignore");
        let matcher = DrailignoreMatcher::from_scope(scope);

        assert!(matcher.is_ignored(&scope.join("generated.gen.rs"), false));
        assert!(matcher.is_ignored(&scope.join("root-only.rs"), false));
        assert!(matcher.is_ignored(&scope.join("ignored-dir/ignored_api.rs"), false));
        assert!(!matcher.is_ignored(&scope.join("ignored-dir/reincluded.rs"), false));
        assert!(!matcher.is_ignored(&scope.join("nested/root-only.rs"), false));
    }
}
