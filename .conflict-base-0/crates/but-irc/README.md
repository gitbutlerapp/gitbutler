# Real-Time Collaboration for Developers

## The Problem

Real-time collaboration around source code remains surprisingly underserved. The tools developers reach for today were built for other audiences:

- **Slack** is optimized for corporate communication — threaded conversations, integrations with HR tools, and pricing models designed for enterprise procurement. It treats code as an afterthought.
- **Discord** was built for gaming communities. It's closer to what developers want, but its identity, moderation model, and feature priorities reflect a consumer social platform, not a development workflow.

Neither tool understands what developers actually do: discuss changes in the context of code, share hunks and commits, react to specific lines, and coordinate work on shared files. The conversation and the code live in entirely separate worlds.

The real cost isn't just the poor fit — it's **context loss**. When a developer asks "why was this changed?" six months later, the answer is buried in a Slack thread nobody can find, a Discord message in the wrong channel, or a local note that only one person ever saw. The knowledge that drove the change evaporates the moment the conversation scrolls off screen.

## The Approach

Chat is a solved problem. It doesn't need another proprietary protocol, another Electron app, or another $8/seat/month subscription. **IRC** — modernized with IRCv3 capabilities like message history, reactions, and rich metadata — provides an experience nearly identical to Discord, with none of the platform lock-in.

My initial focus is teams and small companies where:

- Setup should be minimal — connect and go
- History and context should persist without a SaaS dependency
- The protocol is open, extensible, and well-understood

By embedding IRC directly into GitButler, chat becomes part of the development environment rather than a separate window. Developers see who's working on the same files, react to shared commits, and discuss changes without leaving their workflow.

Because IRC is an open protocol, teams aren't locked in. Anyone can connect with any IRC client alongside GitButler — there's no proprietary wall. This lowers adoption risk to near zero: try it, and if it's not for you, your chat history and workflow still work with standard tooling.

## The Value Proposition

A self-hosted (or GitButler-hosted) IRC server creates something no existing chat tool offers: **a complete, structured record of how code evolves** — from problem identification through debugging to the final solution.

Today, the context behind a change is scattered across Slack threads, GitHub comments, and local notes that nobody else can see. With chat integrated into the version control tool, we capture:

- The conversation that identified the problem
- The hunks and commits shared during debugging
- The reactions and reviews that shaped the solution
- The file-level coordination that prevented conflicts

Because messages are structured — each one has a sender, timestamp, channel, and optional linked data (a commit SHA, a hunk, a file path) — the chat history is straightforward to make queryable. The data is already in a format that lends itself to both embedding-based retrieval and structured context assembly:

- **Embeddings**: messages can be chunked and embedded with their associated code references intact. A similarity search for "auth middleware" returns not just the discussion but the specific commits and hunks that were shared alongside it.
- **Structured context**: because we control the data model, we can construct precise context windows for LLMs — "give me every conversation that references this file, this commit, or this function" — without scraping a third-party API or parsing free-form Slack exports.

The key insight is that the chat history and the code history are stored together, linked by the same identifiers. Building retrieval on top of this is a thin layer, not a data engineering project.

## Bot Extensibility

IRC has a decades-old, battle-tested bot ecosystem. The protocol makes it trivial to build lightweight integrations — no webhook configuration, no OAuth app registration, no API rate limits to navigate.

We already use a dedicated `/bots` channel per project for machine-to-machine communication like working-files broadcasts. This same pattern extends naturally to CI notifications, deployment status, code review bots, and custom team automations. Any team can write a bot in any language that speaks IRC — the barrier to entry is a socket connection and a few lines of code.

## The Service Model

An IRC server is lightweight and isolated by design. Spinning up a separate instance per organization is trivial — each team gets its own server with its own history, its own access controls, and its own data residency.

A 10-person startup signs up. We provision an isolated IRC server in seconds. They open GitButler and they're chatting in their project channel immediately — no admin panel, no seat management, no onboarding wizard.

This maps naturally to a service offering:

- **Self-hosted**: teams run their own IRC server, GitButler connects to it — ideal for regulated industries, air-gapped environments, or teams with strict data residency requirements
- **GitButler-hosted**: we provision an isolated server per organization — zero setup, full data ownership

The per-organization isolation model means there's no shared infrastructure to worry about, no cross-tenant data concerns, and scaling is horizontal by default. And because IRC servers are tiny — a single instance can serve hundreds of users on minimal resources — the operational cost per organization is negligible compared to the per-seat pricing of Slack or Teams.
