//! Mentions written into message content.

use std::collections::HashSet;

/// Extracts unique `@username` mentions from message content, in order of
/// first appearance.
///
/// An `@` only starts a mention at the beginning of the text or after a
/// character that is not alphanumeric, so the `@` in an email address is not
/// one.
pub fn parse(content: &str) -> Vec<String> {
    let mut usernames = Vec::new();
    let mut seen = HashSet::new();
    let chars: Vec<char> = content.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '@' && (i == 0 || !chars[i - 1].is_alphanumeric()) {
            i += 1;
            let start = i;
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            if i > start {
                let username: String = chars[start..i].iter().collect();
                if seen.insert(username.clone()) {
                    usernames.push(username);
                }
            }
        } else {
            i += 1;
        }
    }
    usernames
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_a_mention() {
        assert_eq!(parse("hey @bob check this"), vec!["bob"]);
    }

    #[test]
    fn keeps_order_and_drops_duplicates() {
        assert_eq!(parse("@carol @bob and @carol again"), vec!["carol", "bob"]);
    }

    #[test]
    fn an_email_address_is_not_a_mention() {
        assert!(parse("write to alice@example.com").is_empty());
    }

    #[test]
    fn a_mention_at_the_start_and_after_punctuation() {
        assert_eq!(parse("@alice, (@bob)"), vec!["alice", "bob"]);
    }

    #[test]
    fn a_bare_at_sign_is_not_a_mention() {
        assert!(parse("meet @ noon").is_empty());
    }

    #[test]
    fn underscores_are_part_of_the_name() {
        assert_eq!(parse("@dev_ops"), vec!["dev_ops"]);
    }
}
