//! Edit one frontmatter key in a note's text, keeping everything else.

pub fn set_property(text: &str, key: &str, value: serde_json::Value) -> String {
    let parsed = crate::parse::parse(text);
    let mut map = parsed.frontmatter.clone();
    if value.is_null() {
        map.remove(key);
    } else {
        map.insert(key.to_owned(), value);
    }
    let body = &text[parsed.body_offset..];
    if map.is_empty() {
        return body.to_owned();
    }
    let yaml = serde_yaml_ng::to_string(&map).unwrap_or_default();
    format!("---\n{yaml}---\n{body}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adds_updates_and_removes() {
        let t = set_property("body", "a", serde_json::json!(1));
        assert_eq!(t, "---\na: 1\n---\nbody");
        let t = set_property(&t, "a", serde_json::json!("x"));
        assert_eq!(t, "---\na: x\n---\nbody");
        let t = set_property(&t, "a", serde_json::Value::Null);
        assert_eq!(t, "body");
    }

    #[test]
    fn keeps_other_keys_and_body() {
        let t = set_property("---\nk: v\n---\n# H", "n", serde_json::json!([1, 2]));
        assert_eq!(t, "---\nk: v\nn:\n- 1\n- 2\n---\n# H");
    }
}
