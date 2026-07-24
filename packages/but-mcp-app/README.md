# GitButler workspace MCP App

These React apps render the structured results of the `gitbutler_workspace` and
`gitbutler_review_card` MCP tools. Their compact workspace and review components
take their visual cues from GitButler Lite. Selecting a workspace commit or
branch opens a responsive details panel with copy, explain, and review actions.
Review cards refresh open reviews and their CI status every 30 seconds until the
review closes, CI fails, or a refresh request fails.

Build the single-file resource embedded by the Rust MCP server with:

```sh
pnpm -F @gitbutler/but-mcp-app build
```

The generated `workspace.html` and `review.html` files under
`../../crates/but/src/command/mcp` are committed so normal Cargo builds do not
require Node.js or pnpm.
