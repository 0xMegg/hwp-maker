use std::collections::BTreeMap;

use crate::spec::Spec;

/// Expand `{{name}}` references in all text fields using `spec.vars`.
/// Cells with `placeholder` set are NOT expanded — those carry B-pipeline
/// placeholder syntax and must reach the output unmodified.
pub fn expand(spec: &mut Spec) {
    let vars = spec.vars.clone();
    for cell in spec.table.cells.iter_mut() {
        if cell.placeholder.is_some() {
            continue;
        }
        if let Some(t) = &cell.text {
            cell.text = Some(substitute(t, &vars));
        }
    }
}

pub fn substitute(input: &str, vars: &BTreeMap<String, String>) -> String {
    if !input.contains("{{") {
        return input.to_string();
    }
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(start) = rest.find("{{") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        if let Some(end) = after.find("}}") {
            let key = after[..end].trim();
            // Image placeholders like `image:name` are preserved verbatim
            // so downstream HTML builder can interpret or forward them.
            if key.starts_with("image:") {
                out.push_str(&rest[start..start + 2 + end + 2]);
            } else if let Some(value) = vars.get(key) {
                out.push_str(value);
            } else {
                // Unknown var: leave literal so it's visible in output.
                out.push_str(&rest[start..start + 2 + end + 2]);
            }
            rest = &after[end + 2..];
        } else {
            out.push_str(&rest[start..]);
            rest = "";
            break;
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn substitute_basic() {
        let mut vars = BTreeMap::new();
        vars.insert("name".to_string(), "HOP".to_string());
        assert_eq!(substitute("hello {{name}}!", &vars), "hello HOP!");
    }

    #[test]
    fn substitute_unknown_preserved() {
        let vars = BTreeMap::new();
        assert_eq!(substitute("{{missing}}", &vars), "{{missing}}");
    }

    #[test]
    fn substitute_image_placeholder_preserved() {
        let mut vars = BTreeMap::new();
        vars.insert("logo".to_string(), "substituted".to_string());
        assert_eq!(substitute("{{image:logo}}", &vars), "{{image:logo}}");
    }
}
