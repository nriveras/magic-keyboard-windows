//! Splits a launcher target from app.toml into the file to open and its arguments.

/// `"C:\Program Files\App\app.exe" --profile x` → (`C:\Program Files\App\app.exe`, `--profile x`).
///
/// Unquoted targets split after the first program extension (`.exe`, `.com`, `.bat`, `.cmd`,
/// `.lnk`) followed by a space, so paths with spaces picked in the file dialog still work.
/// Anything else (documents, folders, URLs) is opened as a whole.
pub fn split(target: &str) -> (&str, Option<&str>) {
    fn args(rest: &str) -> Option<&str> {
        Some(rest.trim()).filter(|r| !r.is_empty())
    }
    let target = target.trim();
    if let Some(rest) = target.strip_prefix('"') {
        return match rest.find('"') {
            Some(end) => (&rest[..end], args(&rest[end + 1..])),
            None => (rest, None),
        };
    }
    // ASCII lowercasing keeps byte offsets.
    let lower = target.to_ascii_lowercase();
    for (dot, _) in lower.match_indices('.') {
        for ext in [".exe", ".com", ".bat", ".cmd", ".lnk"] {
            if !lower[dot..].starts_with(ext) {
                continue;
            }
            let end = dot + ext.len();
            if end == target.len() {
                return (target, None);
            }
            if target[end..].starts_with(char::is_whitespace) {
                return (&target[..end], args(&target[end..]));
            }
        }
    }
    (target, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quoted_program_with_arguments() {
        assert_eq!(
            split(r#""C:\Program Files\App\app.exe" --profile x"#),
            (r"C:\Program Files\App\app.exe", Some("--profile x"))
        );
        assert_eq!(
            split(r#""C:\Program Files\App\app.exe""#),
            (r"C:\Program Files\App\app.exe", None)
        );
    }

    #[test]
    fn unquoted_program_path_with_spaces() {
        assert_eq!(
            split(r"C:\Program Files\App\app.exe"),
            (r"C:\Program Files\App\app.exe", None)
        );
        assert_eq!(
            split(r"C:\Program Files\App\App.EXE  -new-window "),
            (r"C:\Program Files\App\App.EXE", Some("-new-window"))
        );
        assert_eq!(
            split(r"notepad.exe C:\notes\todo.txt"),
            ("notepad.exe", Some(r"C:\notes\todo.txt"))
        );
        assert_eq!(
            split(r"C:\Tools\run.cmd fast"),
            (r"C:\Tools\run.cmd", Some("fast"))
        );
    }

    #[test]
    fn documents_folders_and_urls_open_whole() {
        for target in [
            r"C:\My Documents\report.pdf",
            r"C:\Users\me\Desktop",
            "https://example.com/a b",
            "ms-settings:bluetooth",
            r"C:\Apps\my.executable notes\file.txt",
        ] {
            assert_eq!(split(target), (target, None), "{target}");
        }
    }

    #[test]
    fn surrounding_whitespace_is_ignored() {
        assert_eq!(split("  calc.exe  "), ("calc.exe", None));
    }
}
