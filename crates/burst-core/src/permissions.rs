//! Who may do what in a channel.
//!
//! Pure. Handlers load the caller's instance role and their role in the
//! channel, then ask. Keeping every decision here means the whole matrix is
//! read, and tested, in one place.

use crate::models::channel::ChannelMemberRole;

/// A user's role across the instance, as stored in `users.role`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstanceRole {
    Admin,
    Integrator,
    /// A moderator in every channel they belong to.
    Moderator,
    Member,
    /// Reads and sends only in channels someone added them to.
    Guest,
}

impl InstanceRole {
    /// Parses a stored role. The API accepts only the five values; anything
    /// else is read as `Guest`, the least a caller can be granted.
    pub fn parse(value: &str) -> Self {
        match value {
            "admin" => Self::Admin,
            "integrator" => Self::Integrator,
            "moderator" => Self::Moderator,
            "member" => Self::Member,
            _ => Self::Guest,
        }
    }
}

/// The caller, as far as a channel decision is concerned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Actor {
    pub instance: InstanceRole,
    /// Their role in the channel, or `None` when they are not a member.
    pub channel: Option<ChannelMemberRole>,
}

/// Decisions that depend only on who the caller is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Create a public or private channel.
    CreateChannel,
    /// Open a direct or group message with other users.
    StartDirectMessage,
    /// List the public channels they could join.
    BrowsePublicChannels,
    /// Read a public channel's details without being a member.
    ViewPublicChannel,
    /// Add themselves to a public channel.
    JoinPublicChannel,
    /// Add another user to this channel.
    AddMember,
    /// Change this channel's name, topic or description.
    EditChannel,
    /// Pin or unpin a message in this channel.
    PinMessage,
    /// Delete a message someone else wrote in this channel.
    DeleteOthersMessage,
    /// Archive or unarchive this channel.
    ArchiveChannel,
}

impl Actor {
    fn is_admin(&self) -> bool {
        self.instance == InstanceRole::Admin
    }

    fn is_guest(&self) -> bool {
        self.instance == InstanceRole::Guest
    }

    fn is_member(&self) -> bool {
        self.channel.is_some()
    }

    fn is_owner(&self) -> bool {
        self.channel == Some(ChannelMemberRole::Owner)
    }

    /// Moderates this channel: its owner, a member made its moderator, or an
    /// instance moderator who belongs to it. Moderation is per channel, so an
    /// instance moderator has no reach into a channel they are not in. A guest
    /// never moderates, whatever their channel role says.
    fn moderates(&self) -> bool {
        if self.is_guest() {
            return false;
        }
        match self.channel {
            Some(ChannelMemberRole::Owner | ChannelMemberRole::Moderator) => true,
            Some(ChannelMemberRole::Member) => self.instance == InstanceRole::Moderator,
            None => false,
        }
    }

    /// Standing in the channel for removals. An instance moderator holds the
    /// channel role `member` but moderates, so they rank as a moderator: equal
    /// to a channel moderator, above a member.
    fn rank(&self) -> u8 {
        match self.channel {
            Some(ChannelMemberRole::Member) if self.moderates() => {
                rank(Some(ChannelMemberRole::Moderator))
            }
            other => rank(other),
        }
    }

    /// Whether the caller may do `action`.
    pub fn can(&self, action: Action) -> bool {
        if self.is_admin() {
            return true;
        }
        match action {
            Action::CreateChannel
            | Action::StartDirectMessage
            | Action::BrowsePublicChannels
            | Action::ViewPublicChannel
            | Action::JoinPublicChannel => !self.is_guest(),
            Action::AddMember | Action::EditChannel | Action::PinMessage => {
                self.is_member() && !self.is_guest()
            }
            Action::DeleteOthersMessage | Action::ArchiveChannel => self.moderates(),
        }
    }

    /// Whether the caller may remove a member whose channel role is `target`.
    ///
    /// Nobody removes the owner: a channel always has one, and leaving is the
    /// owner's own decision. Otherwise the caller must moderate the channel and
    /// outrank the target, so moderators cannot remove each other.
    pub fn can_remove(&self, target: ChannelMemberRole) -> bool {
        if target == ChannelMemberRole::Owner {
            return false;
        }
        if self.is_admin() {
            return true;
        }
        self.moderates() && self.rank() > rank(Some(target))
    }

    /// Whether the caller may change a member from `current` to `new`.
    ///
    /// Only the owner, or an admin, appoints and removes moderators. Ownership
    /// is not transferred this way, and the owner's own role cannot change.
    pub fn can_set_role(&self, current: ChannelMemberRole, new: ChannelMemberRole) -> bool {
        if current == ChannelMemberRole::Owner || new == ChannelMemberRole::Owner {
            return false;
        }
        self.is_admin() || self.is_owner()
    }
}

fn rank(role: Option<ChannelMemberRole>) -> u8 {
    match role {
        Some(ChannelMemberRole::Owner) => 3,
        Some(ChannelMemberRole::Moderator) => 2,
        Some(ChannelMemberRole::Member) => 1,
        None => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ChannelMemberRole::{Member, Moderator, Owner};

    const ALL_ACTIONS: [Action; 10] = [
        Action::CreateChannel,
        Action::StartDirectMessage,
        Action::BrowsePublicChannels,
        Action::ViewPublicChannel,
        Action::JoinPublicChannel,
        Action::AddMember,
        Action::EditChannel,
        Action::PinMessage,
        Action::DeleteOthersMessage,
        Action::ArchiveChannel,
    ];

    fn actor(instance: InstanceRole, channel: Option<ChannelMemberRole>) -> Actor {
        Actor { instance, channel }
    }

    #[test]
    fn an_admin_may_do_everything_even_outside_the_channel() {
        let admin = actor(InstanceRole::Admin, None);
        for action in ALL_ACTIONS {
            assert!(admin.can(action), "{action:?}");
        }
    }

    #[test]
    fn a_guest_member_may_do_nothing_beyond_reading_and_sending() {
        let guest = actor(InstanceRole::Guest, Some(Member));
        for action in ALL_ACTIONS {
            assert!(!guest.can(action), "a guest may not {action:?}");
        }
    }

    #[test]
    fn a_guest_cannot_find_or_enter_channels_on_their_own() {
        let outsider = actor(InstanceRole::Guest, None);
        assert!(!outsider.can(Action::BrowsePublicChannels));
        assert!(!outsider.can(Action::ViewPublicChannel));
        assert!(!outsider.can(Action::JoinPublicChannel));
        assert!(!outsider.can(Action::CreateChannel));
        assert!(!outsider.can(Action::StartDirectMessage));
    }

    #[test]
    fn a_member_keeps_what_members_could_already_do() {
        let member = actor(InstanceRole::Member, Some(Member));
        for action in [
            Action::CreateChannel,
            Action::StartDirectMessage,
            Action::BrowsePublicChannels,
            Action::JoinPublicChannel,
            Action::AddMember,
            Action::EditChannel,
            Action::PinMessage,
        ] {
            assert!(member.can(action), "{action:?}");
        }
    }

    #[test]
    fn a_member_does_not_moderate() {
        let member = actor(InstanceRole::Member, Some(Member));
        assert!(!member.can(Action::DeleteOthersMessage));
        assert!(!member.can(Action::ArchiveChannel));
        assert!(!member.can_remove(Member));
    }

    #[test]
    fn channel_actions_need_membership() {
        let outsider = actor(InstanceRole::Member, None);
        assert!(!outsider.can(Action::AddMember));
        assert!(!outsider.can(Action::EditChannel));
        assert!(!outsider.can(Action::PinMessage));
    }

    #[test]
    fn the_owner_and_a_channel_moderator_moderate() {
        for role in [Owner, Moderator] {
            let a = actor(InstanceRole::Member, Some(role));
            assert!(a.can(Action::DeleteOthersMessage), "{role:?}");
            assert!(a.can(Action::ArchiveChannel), "{role:?}");
            assert!(a.can_remove(Member), "{role:?}");
        }
    }

    #[test]
    fn an_instance_moderator_moderates_only_channels_they_belong_to() {
        let inside = actor(InstanceRole::Moderator, Some(Member));
        assert!(inside.can(Action::DeleteOthersMessage));
        assert!(inside.can_remove(Member));

        let outside = actor(InstanceRole::Moderator, None);
        assert!(!outside.can(Action::DeleteOthersMessage));
        assert!(!outside.can_remove(Member));
    }

    #[test]
    fn nobody_removes_the_owner() {
        for a in [
            actor(InstanceRole::Admin, None),
            actor(InstanceRole::Member, Some(Owner)),
            actor(InstanceRole::Moderator, Some(Moderator)),
        ] {
            assert!(!a.can_remove(Owner), "{a:?}");
        }
    }

    #[test]
    fn moderators_cannot_remove_each_other() {
        let moderator = actor(InstanceRole::Member, Some(Moderator));
        assert!(!moderator.can_remove(Moderator));
        let owner = actor(InstanceRole::Member, Some(Owner));
        assert!(owner.can_remove(Moderator));
    }

    #[test]
    fn an_instance_moderator_cannot_remove_a_channel_moderator() {
        let instance_moderator = actor(InstanceRole::Moderator, Some(Member));
        assert!(!instance_moderator.can_remove(Moderator));
    }

    #[test]
    fn only_the_owner_or_an_admin_appoints_moderators() {
        assert!(actor(InstanceRole::Member, Some(Owner)).can_set_role(Member, Moderator));
        assert!(actor(InstanceRole::Admin, None).can_set_role(Moderator, Member));
        assert!(!actor(InstanceRole::Member, Some(Moderator)).can_set_role(Member, Moderator));
        assert!(!actor(InstanceRole::Moderator, Some(Member)).can_set_role(Member, Moderator));
    }

    #[test]
    fn ownership_cannot_be_given_or_taken_by_changing_a_role() {
        let owner = actor(InstanceRole::Member, Some(Owner));
        let admin = actor(InstanceRole::Admin, None);
        assert!(!owner.can_set_role(Member, Owner));
        assert!(!admin.can_set_role(Owner, Member));
    }

    #[test]
    fn a_guest_made_a_channel_moderator_still_does_not_moderate() {
        let guest = actor(InstanceRole::Guest, Some(Moderator));
        assert!(!guest.can(Action::DeleteOthersMessage));
        assert!(!guest.can(Action::ArchiveChannel));
        assert!(!guest.can_remove(Member));
    }

    #[test]
    fn an_unknown_instance_role_is_the_least_privileged() {
        assert_eq!(InstanceRole::parse("superuser"), InstanceRole::Guest);
        assert_eq!(InstanceRole::parse("admin"), InstanceRole::Admin);
        assert_eq!(InstanceRole::parse("moderator"), InstanceRole::Moderator);
    }
}
