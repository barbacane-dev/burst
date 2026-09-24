# Channels & Messaging

Channels are where team conversations happen. Every channel has a name, an optional topic, and a member list.

## Channel Types

- **Public**: visible to everyone except guests. Anyone else can browse and join.
- **Private**: visible only to members. The only way in is to be added by a member.

## Creating a Channel

Click the **+** icon next to "Channels" in the sidebar. Enter a channel name and click **Create**. The name is automatically converted to a URL-friendly slug (e.g., "Engineering Team" becomes `engineering-team`).

You become the channel **owner** with full control over the channel.

## Browsing and Joining

Click the search icon next to "Channels" to browse all public channels. Each channel shows its name and topic. Click **Join** to become a member.

Private channels do not appear in the browse list.

## Adding People

Any member of a channel can add someone to it, except a guest. Adding someone is the only way into a private channel, and the only way a guest enters any channel. Deactivated users cannot be added.

Click the people icon in the channel header to open the **Members** panel, then **Add people** and choose who to add. The panel lists everyone in the channel with their role.

## Channel Topic and Description

Members can update the topic and description via the channel settings. Guests cannot. The topic appears in the channel header as a quick reference.

## Leaving a Channel

Open the **Members** panel and select **Leave channel**. You can rejoin public channels at any time from the browse dialog.

## Moderators

A channel's owner can make any member a **moderator** of that channel, and make them a member again. Both are in the **Members** panel, beside each member. A moderator can:

- delete anyone's message in the channel, which is recorded in the [audit log](../admin/audit-log.md);
- remove members from the channel;
- archive and unarchive it.

Users with the instance-wide **moderator** role moderate every channel they are a member of, and no others. Nobody can remove a channel's owner, and moderators cannot remove one another; the owner can remove a moderator. A guest cannot be made a moderator.

## Archiving

Anyone who moderates the channel, and admins, can archive it, making it **read-only**. Members can still read the message history but cannot send new messages.

To archive: open the **Members** panel and select **Archive channel**, or use the admin panel. Archived channels can be unarchived the same way.

## Unread Counts

The sidebar shows a badge with the number of unread messages for each channel. Visiting a channel marks it as read and clears the badge.
