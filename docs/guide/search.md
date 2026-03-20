# Search

Burst provides full-text search across all channels you're a member of.

## Opening Search

Press `Cmd+K` (macOS) or `Ctrl+K` (Windows/Linux), or click the search icon in the sidebar header. A search dialog appears.

## Searching

Type your query. Results appear after a short debounce (300ms). Each result shows:

- The author's display name
- The date
- A snippet with matching terms **highlighted**

Search uses PostgreSQL full-text search, which understands word stems (e.g., searching "running" also matches "run").

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

- Keep queries short — one or two keywords work best.
- Use `Esc` to close the search dialog without navigating.
