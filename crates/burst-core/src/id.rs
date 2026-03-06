use uuid::Uuid;

pub fn new_id() -> Uuid {
    Uuid::now_v7()
}

pub fn format_user_id(id: Uuid) -> String {
    format!("usr_{id}")
}

pub fn parse_prefixed_id(s: &str, prefix: &str) -> Option<Uuid> {
    s.strip_prefix(prefix)
        .and_then(|id| Uuid::parse_str(id).ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_id_is_v7() {
        let id = new_id();
        assert_eq!(id.get_version(), Some(uuid::Version::SortRand));
    }

    #[test]
    fn format_and_parse_roundtrip() {
        let id = new_id();
        let formatted = format_user_id(id);
        assert!(formatted.starts_with("usr_"));
        let parsed = parse_prefixed_id(&formatted, "usr_").unwrap();
        assert_eq!(id, parsed);
    }
}
