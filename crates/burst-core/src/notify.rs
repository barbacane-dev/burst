//! Who is notified of a new message, and why.
//!
//! Pure: the server loads a channel's members with their preference, the
//! message author and the users it mentions, and delivers what this returns.

use std::collections::HashSet;

use uuid::Uuid;

/// A member's notification preference for one channel, as stored in
/// `channel_members.notify`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preference {
    /// Every message.
    All,
    /// Only messages that mention the member.
    Mentions,
    /// Nothing, mentions included.
    Nothing,
}

impl Preference {
    /// Parses a stored preference. The API accepts only the three values, so
    /// anything else is treated as the column default, `all`: a preference the
    /// server cannot read should not silence a member.
    pub fn parse(value: &str) -> Self {
        match value {
            "mentions" => Self::Mentions,
            "nothing" => Self::Nothing,
            _ => Self::All,
        }
    }
}

/// Why a member is notified.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    /// The message mentions them.
    Mention,
    /// Their preference asks for every message.
    Message,
}

impl Reason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Mention => "mention",
            Self::Message => "message",
        }
    }
}

/// The members to notify of a message, with the reason for each.
///
/// The author is never notified of their own message. `Nothing` silences a
/// member even when mentioned; `Mentions` notifies only when mentioned; `All`
/// notifies always, reporting a mention as the reason when there is one. A
/// mentioned user who is not a member is not notified: a member list is the
/// only authority on who may read the channel.
pub fn recipients(
    members: &[(Uuid, Preference)],
    author: Uuid,
    mentioned: &HashSet<Uuid>,
) -> Vec<(Uuid, Reason)> {
    members
        .iter()
        .filter(|(user, _)| *user != author)
        .filter_map(|&(user, preference)| {
            let is_mentioned = mentioned.contains(&user);
            match (preference, is_mentioned) {
                (Preference::Nothing, _) => None,
                (_, true) => Some((user, Reason::Mention)),
                (Preference::All, false) => Some((user, Reason::Message)),
                (Preference::Mentions, false) => None,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(n: u128) -> Uuid {
        Uuid::from_u128(n)
    }

    const AUTHOR: u128 = 1;

    fn route(members: &[(u128, Preference)], mentioned: &[u128]) -> Vec<(Uuid, Reason)> {
        let members: Vec<_> = members.iter().map(|&(n, p)| (id(n), p)).collect();
        let mentioned: HashSet<_> = mentioned.iter().map(|&n| id(n)).collect();
        recipients(&members, id(AUTHOR), &mentioned)
    }

    #[test]
    fn the_author_is_never_notified() {
        let out = route(&[(AUTHOR, Preference::All)], &[AUTHOR]);
        assert!(out.is_empty(), "{out:?}");
    }

    #[test]
    fn all_notifies_on_every_message() {
        assert_eq!(
            route(&[(AUTHOR, Preference::All), (2, Preference::All)], &[]),
            vec![(id(2), Reason::Message)]
        );
    }

    #[test]
    fn all_reports_a_mention_as_the_reason() {
        assert_eq!(
            route(&[(2, Preference::All)], &[2]),
            vec![(id(2), Reason::Mention)]
        );
    }

    #[test]
    fn mentions_only_notifies_when_mentioned() {
        assert!(route(&[(2, Preference::Mentions)], &[]).is_empty());
        assert_eq!(
            route(&[(2, Preference::Mentions)], &[2]),
            vec![(id(2), Reason::Mention)]
        );
    }

    #[test]
    fn nothing_silences_even_a_mention() {
        assert!(route(&[(2, Preference::Nothing)], &[2]).is_empty());
    }

    #[test]
    fn a_mentioned_non_member_is_not_notified() {
        assert!(
            route(&[(2, Preference::All)], &[3])
                .iter()
                .all(|(u, _)| *u != id(3))
        );
    }

    #[test]
    fn each_member_is_decided_on_their_own_preference() {
        let out = route(
            &[
                (AUTHOR, Preference::All),
                (2, Preference::All),
                (3, Preference::Mentions),
                (4, Preference::Mentions),
                (5, Preference::Nothing),
            ],
            &[4, 5],
        );
        assert_eq!(
            out,
            vec![(id(2), Reason::Message), (id(4), Reason::Mention)]
        );
    }

    #[test]
    fn an_unreadable_preference_behaves_as_the_default() {
        assert_eq!(Preference::parse("mentions"), Preference::Mentions);
        assert_eq!(Preference::parse("nothing"), Preference::Nothing);
        assert_eq!(Preference::parse("all"), Preference::All);
        assert_eq!(Preference::parse("garbage"), Preference::All);
    }
}
