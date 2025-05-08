# GitButler MCP

A Model Context Protocol server implementation for GitButler that provides AI assistants with branch management capabilities.

## Overview

`gitbutler-mcp` is a Rust crate that implements a Model Context Protocol (MCP) server for GitButler. It enables AI assistants to perform branch-related operations in GitButler repositories through standardized MCP interactions.

## Features

- **Branch Management**: Update branches with summaries and prompts
- **MCP Compliance**: Fully implements the Model Context Protocol specification
- **Tooling Integration**: Provides tools that can be used by AI assistants

## Usage

The MCP server can be integrated with AI assistants that support the Model Context Protocol. It exposes tools that allow these assistants to:

- Update branches with contextual information
- Process prompts and convert them into branch-specific actions

## Tool Reference

### `update_branch`

Updates a GitButler branch with a given prompt and summary.

**Parameters:**
- `working_directory`: Path to the Git repository
- `full_prompt`: Complete prompt that was used for the branch update
- `summary`: Short description of the changes

## Development

To run the MCP server:

```bash
cargo run -p gitbutler-mcp
```

## Integration

This MCP server can be integrated with any AI assistant that supports the Model Context Protocol (MCP) specification.

## License

Same as the GitButler project.