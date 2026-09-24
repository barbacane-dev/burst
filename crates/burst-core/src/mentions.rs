//! Mentions written into message content.

use std::collections::HashSet;

/// `@channel`: every member of the channel.
pub const CHANNEL: &str = "channel";
/// `@here`: the members of the channel who are online when the message is sent.
pub const HERE: &str = "here";

/// The mentions found in a message.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Mentions {
    /// `@username` mentions, unique, in order of first appearance.
    pub users: Vec<String>,
    /// Whether `@channel` appears.
    pub channel: bool,
    /// Whether `@here` appears.
    pub here: bool,
}

impl Mentions {
    /// Whether the message addresses more than the users it names.
    pub fn is_broadcast(&self) -> bool {
        self.channel || self.here
    }
}

/// Extracts the mentions in message content.
///
/// An `@` only starts a mention at the beginning of the text or after a
/// character that is not alphanumeric, so the `@` in an email address is not
/// one. `@channel` and `@here` are matched case-insensitively and are never
/// read as usernames, so a user of either name cannot be mentioned alone.
pub fn parse(content: &str) -> Mentions {
    let mut mentions = Mentions::default();
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
                let name: String = chars[start..i].iter().collect();
                if name.eq_ignore_ascii_case(CHANNEL) {
                    mentions.channel = true;
                } else if name.eq_ignore_ascii_case(HERE) {
                    mentions.here = true;
                } else if seen.insert(name.clone()) {
                    mentions.users.push(name);
                }
            }
        } else {
            i += 1;
        }
    }
    mentions
}

#[cfg(test)]
mod tests {
    use super::*;

    fn users(content: &str) -> Vec<String> {
        parse(content).users
    }

    #[test]
    fn finds_a_mention() {
        assert_eq!(users("hey @bob check this"), vec!["bob"]);
    }

    #[test]
    fn keeps_order_and_drops_duplicates() {
        assert_eq!(users("@carol @bob and @carol again"), vec!["carol", "bob"]);
    }

    #[test]
    fn an_email_address_is_not_a_mention() {
        assert_eq!(parse("write to alice@example.com"), Mentions::default());
    }

    #[test]
    fn a_mention_at_the_start_and_after_punctuation() {
        assert_eq!(users("@alice, (@bob)"), vec!["alice", "bob"]);
    }

    #[test]
    fn a_bare_at_sign_is_not_a_mention() {
        assert_eq!(parse("meet @ noon"), Mentions::default());
    }

    #[test]
    fn underscores_are_part_of_the_name() {
        assert_eq!(users("@dev_ops"), vec!["dev_ops"]);
    }

    #[test]
    fn channel_and_here_are_broadcasts_not_users() {
        let m = parse("@channel and @here, also @bob");
        assert!(m.channel);
        assert!(m.here);
        assert_eq!(m.users, vec!["bob"]);
        assert!(m.is_broadcast());
    }

    #[test]
    fn broadcasts_are_case_insensitive() {
        assert!(parse("@Channel").channel);
        assert!(parse("@HERE").here);
    }

    #[test]
    fn a_longer_word_starting_with_a_broadcast_is_a_user() {
        let m = parse("@channels @hereford");
        assert!(!m.is_broadcast());
        assert_eq!(m.users, vec!["channels", "hereford"]);
    }

    #[test]
    fn plain_text_is_not_a_broadcast() {
        let m = parse("everyone is here in the channel");
        assert!(!m.is_broadcast());
    }
}
