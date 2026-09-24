# Search

Burst provides full-text search across all channels you're a member of.

## Opening Search

Press `Cmd+K` (macOS) or `Ctrl+K` (Windows/Linux), or click the search icon in the sidebar header. A search dialog appears.

## Searching

Type your query. Results appear after a short debounce (300ms). Each result shows:

- The author's display name
- The date
- A snippet with matching terms **highlighted**
- The file name, when the message was found through one of its attachments

Search uses PostgreSQL full-text search, which understands word stems (e.g., searching "running" also matches "run").

Results are ordered by relevance. Click **Show more results** at the bottom of the list to load the next page.

## File Names

Search also matches the names of files attached to messages, anywhere in the name and regardless of case: `report` finds `Q3-Report-final.pdf`. Characters such as `%` and `_` match themselves.

## Filters

The row under the search box narrows results:

| Filter | Keeps |
|--------|-------|
| Author | Messages written by one person |
| Sent ... to ... | Messages sent between two days, both included, in your own time zone. Either day can be left empty |
| Has file | Messages with at least one attachment |

Changing a filter runs the search again. A range whose start is after its end is not searched.

## Navigating to Results

Click any result to navigate to that message's channel. The search dialog closes automatically.

## Quoted Phrases

Wrap your query in double quotes for exact phrase matching:

```
"deployment pipeline"
```

## Scope

Search returns results from all channels where you're a member. You cannot search channels you haven't joined. DMs are included in search results.

## Tips

- Keep queries short: one or two keywords work best.
- Use `Esc` to close the search dialog without navigating.
