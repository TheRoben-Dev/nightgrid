//! POSIX path helpers for remote paths (SFTP always uses `/`).

pub fn posix_join(dir: &str, name: &str) -> String {
    if name.starts_with('/') || dir.is_empty() {
        name.to_owned()
    } else if dir.ends_with('/') {
        format!("{dir}{name}")
    } else {
        format!("{dir}/{name}")
    }
}

pub fn posix_file_name(path: &str) -> &str {
    let t = path.trim_end_matches('/');
    if t.is_empty() {
        return "/";
    }
    t.rsplit('/').next().unwrap_or(t)
}

/// Parent directory; `None` at the root.
pub fn posix_parent(path: &str) -> Option<String> {
    let t = path.trim_end_matches('/');
    if t.is_empty() {
        return None;
    }
    match t.rfind('/') {
        Some(0) => Some("/".into()),
        Some(i) => Some(t[..i].to_owned()),
        None => Some(".".into()),
    }
}

/// Quotes a string for a POSIX shell (`'...'`, with `'` → `'\''`).
pub fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn join_parent_name() {
        assert_eq!(posix_join("/opt", "stacks"), "/opt/stacks");
        assert_eq!(posix_join("/", "etc"), "/etc");
        assert_eq!(posix_join("/opt/", "x"), "/opt/x");
        assert_eq!(posix_join("/opt", "/abs"), "/abs");
        assert_eq!(posix_parent("/opt/stacks"), Some("/opt".into()));
        assert_eq!(posix_parent("/opt/"), Some("/".into()));
        assert_eq!(posix_parent("/"), None);
        assert_eq!(posix_parent("rel"), Some(".".into()));
        assert_eq!(posix_file_name("/opt/stacks/"), "stacks");
        assert_eq!(posix_file_name("/"), "/");
    }

    #[test]
    fn quoting() {
        assert_eq!(shell_quote("/srv/it's here"), r"'/srv/it'\''s here'");
        assert_eq!(shell_quote("$(rm -rf /)"), "'$(rm -rf /)'");
    }
}
