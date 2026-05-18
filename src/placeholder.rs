/// Parses a cell's text to detect B-방식 placeholder syntax.
///
/// - `{{key}}`           → `Some(Ph::Text("key"))`
/// - `{{image:key}}`     → `Some(Ph::Image("key"))`
/// - anything else       → `None`
///
/// Matching is **strict**: the entire trimmed input must be a single placeholder.
/// Partial matches are rejected to prevent accidental style damage.
#[derive(Debug, PartialEq, Eq)]
pub enum Ph<'a> {
    Text(&'a str),
    Image(&'a str),
}

pub fn parse(input: &str) -> Option<Ph<'_>> {
    let s = input.trim();
    if !s.starts_with("{{") || !s.ends_with("}}") {
        return None;
    }
    let inner = &s[2..s.len() - 2].trim();
    if inner.is_empty() {
        return None;
    }
    if let Some(rest) = inner.strip_prefix("image:") {
        let name = rest.trim();
        if name.is_empty() {
            return None;
        }
        return Some(Ph::Image(trim_to_bytes(inner, "image:".len(), name)));
    }
    // text placeholder: ensure no spaces / colons in the key (strict form)
    if inner.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-') {
        return Some(Ph::Text(trim_to_bytes(inner, 0, inner)));
    }
    None
}

/// Re-slice `inner` to produce a `&str` of `name` with the original lifetime.
/// Callers pass the absolute offset within `inner` and the borrowed `&str` that
/// must match the suffix. (Trims matched against lifetime of original input.)
fn trim_to_bytes<'a>(full: &'a str, offset: usize, expected: &str) -> &'a str {
    &full[offset..offset + expected.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_plain() {
        assert_eq!(parse("{{name}}"), Some(Ph::Text("name")));
    }

    #[test]
    fn text_trims_surrounding_ws() {
        assert_eq!(parse("  {{name}}  "), Some(Ph::Text("name")));
    }

    #[test]
    fn image_placeholder() {
        assert_eq!(parse("{{image:logo}}"), Some(Ph::Image("logo")));
    }

    #[test]
    fn rejects_partial() {
        assert_eq!(parse("prefix {{name}}"), None);
        assert_eq!(parse("{{name}} suffix"), None);
    }

    #[test]
    fn rejects_invalid() {
        assert_eq!(parse(""), None);
        assert_eq!(parse("{{}}"), None);
        assert_eq!(parse("{{has space}}"), None);
        assert_eq!(parse("{{image:}}"), None);
    }
}
