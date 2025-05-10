# Butler MCP

This crate implments a single binary Rust MCP server that can be pointed to by an Agent to do some basic branch management work with the GitButler tooling.

It implements a single tool called 'update_branch' that will look at uncommitted changes in the working directory and either commit them or amend an existing commit. 

If there is no existing branch, it will create a new one.

If AI capabilities are enabled, it will also use the AI to generate a commit message for the changes based on the prompt.

## The Idea

The concept is not to give an Agent an endpoint to every API that we have, which mainly results in being able to use the agent as a slow command line. The idea is to have a few very powerful tools that can be used to do a lot of work automatically. 

Most of the work should be done in GitButler for more specific tasks, but updating a branch with new work generated via agentic work can be simple and powerful.

## TODO

- [ ] create a new branch if there is no existing one
- [ ] determine the actual branch name to use of everything existing
- [ ] determine if a new virtual branch should be created
- [ ] determine if work should be committed or amended
- [ ] use the AI to generate a commit message