#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Profile {
    pub byte_len: usize,
    pub line_count: usize,
    pub max_line_len: usize,
    pub newline_count: usize,
}

impl Profile {
    #[must_use]
    pub fn is_likely_minified(self) -> bool {
        let oversized_single_line = self.max_line_len >= 120;
        let few_lines = self.line_count <= 3;
        let low_newline_density = self.newline_count.saturating_mul(100) <= self.byte_len;

        oversized_single_line && (few_lines || low_newline_density)
    }
}

#[must_use]
pub fn profile(content: &str) -> Profile {
    let mut max_line_len = 0;
    let mut line_len = 0;
    let mut newline_count = 0;

    for ch in content.chars() {
        if ch == '\n' {
            max_line_len = max_line_len.max(line_len);
            line_len = 0;
            newline_count += 1;
        } else {
            line_len += ch.len_utf8();
        }
    }

    max_line_len = max_line_len.max(line_len);

    Profile {
        byte_len: content.len(),
        line_count: newline_count + 1,
        max_line_len,
        newline_count,
    }
}

#[cfg(test)]
mod tests {
    use super::{profile, Profile};

    #[test]
    fn detects_minified_single_line_bundle() {
        let profile = profile(
            "export function stableEntryPoint(n){function internalMixer(x){return x*2+1}return internalMixer(n)+internalMixer(3)}stableEntryPoint(5);",
        );

        assert!(profile.is_likely_minified());
    }

    #[test]
    fn ignores_normal_short_content() {
        let profile = profile("fn main() {\n    println!(\"ok\");\n}\n");

        assert_eq!(
            profile,
            Profile {
                byte_len: 34,
                line_count: 4,
                max_line_len: 19,
                newline_count: 3
            }
        );
        assert!(!profile.is_likely_minified());
    }
}
