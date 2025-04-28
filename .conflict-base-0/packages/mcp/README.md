# GitButler MCP server

> **Note**  
> This is still in development, mainly because this server requires make some resources available.

This MCP server brings functionality from the GitButler review API and GitButler client.

In order to access the former, you'll need a **GitButler API key**.
For the former, you'll need the path to the **but-cli** binary.

These are currently only available to internal users, which is why this is considered 'in-development'.

### Dev Configuration

1. Checkout this repository locally

2. At the root, run the following command to build the but-cli:

```bash
# Build the CLI.
# This should create a binary in the **target/debug** directory.
cargo build -p but-cli
```

3. Go to the **packages/mcp** directory and run the following command:

```bash
# Install the dependencies.
pnpm i
# Bundle the MCP server script.
# This will create the bundle under **build/index.js**
pnpm build
```

4. Hook it up to your MCP client's list of servers. This is how it would look for VSCode:\
   You **don't need** to set both the API key and the executable path, you just need either.
   If you set both, you'll have access to all the tools for both review and the client.

```json
    "mcp": {
        "servers": {
            "gitbutler": {
                "command": "node",
                "args": ["/Absolute/path/to/gitbutler/packages/mcp/build/index.js"],
                "env": {
                    "GITBUTLER_API_KEY": "MY_API_KEY",
                    "GITBUTLER_EXECUTABLE_PATH": "/Absolute/path/to/gitbutler/target/debug/but-cli"
                }
            }
        }
    },
```

5. Start the server.\
   Depending on what environment variables you've set, you're MCP client will see different sets of tools
