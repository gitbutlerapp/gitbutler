## General information

This is a monorepo with multiple projects.
The main applications are found in the `apps` directory.
They are:

- `desktop` containing the Tauri application's frontend code
- `web` containing the web application's frontend code

The backend of the Tauri application is found in the `crates` directory.
It contains different rust packages, all used for the Tauri application.

The `packages` directory contains different self-contained npm packages.
These are shared between the `desktop` and `web` applications.
The packages are:

- `ui` containing the shared UI components
- `shared` containing the shared types and utils
- `mcp` containing the Model Context Protocol packages
- `no-relative-imports` containing the no-relative-imports ESLINT package

## Version control

- Use GitButler tools
- The MCP tools require the absolute path to the repository
- Don't use any other git commands

### Absorb

When told to 'absorb' follow these steps:

1. If there were any instructions given, take them into account.
2. List the file changes
3. Get the hunk dependencies
4. For all files that depend on a **single** commit, amend the file onto that commit.
5. If there are no dependencies, list the stacks. Based on the stack branch names and descriptions, determine the best branch to commit the changes to.
6. List the commits in the branch determined in the previous step, and then determine the best commit to amend the changes to based on the description. Update the description if needed.

### Figure out the commits

When told to 'figure out the commits' follow these steps:

1. List the file changes
2. List the stacks
3. Figure out, based on the changes and whether there are any applied stacks/branches:
4. Create a plan for the commits. For that, take a look at the `Create a commit plan` section below.
5. Always execute the plan, and commit the changes as previously determined unless otherwise directed.

### Creating a commit plan

Follow this instructions when creating a commit plan:

1. Group the file changes into logical groups
   - Take a look at their diffs and determine if they are related.
   - Groups are good, but prefer to have smaller commits than larger ones.
   - Granularity is good.
2. Determine if any branches should be created
   - If there are any stacks with branches, take a look at the branch names and descriptions and match them with the file changes.
   - Create multiple branches if needed. You can tell if multiple branches are needed if the file changes span multiple projects in the monorepo.
3. Determine the commits
   - For each group of file changes, determine the commit message. Be descriptive and explain what the changes are.
   - If branches need to be created, use a descriptive name for the branch.
   - Determine the order of the commits. If there are any dependencies between the commits, make sure to commit them in the correct order.
   - Define which commits should go into which branches.
   - If there were any other instructions given, take them into account.

In the end, the plan should contain a list of the branches to be created (if any) and the commits to be made
