# Dependencies

JavaScript dependencies are sourced from pnpm. Commands are surfaced via pnpm.

# Automation

In dev the app is accessible for agent automation on port 9222. A working
CDP driver script and its gotchas are in
`.agents/skills/lite-render-perf/SKILL.md` under "Driving the dev app over
CDP".

# Components

Memoization utilities such as `useMemo`, `useCallback`, and `React.memo` are usually redundant as we use React Compiler, however may be necessary in hot paths where the compiler fails to understand that a computation is pure and therefore safe to memoise.

The compiler does not prevent re-render regressions: it silently skips memoizing calls to imported functions, and context still re-renders every consumer. Before writing code that derives values during render, adds a context, subscribes to the store, or renders lists of rows — or when the UI is slow or re-renders too much — use the `lite-render-perf` skill (`.agents/skills/lite-render-perf/SKILL.md`).

Component definitions should follow this pattern, optionally destructuring `p`:

```tsx
type Props = {
  ...
};

export const MyComponent: FC<Props> = (p) => {
  // [...]
};
```

# State

Share machinery, not state: when a new surface (a tab, pane, or mode) has its
own selection or lifecycle, give it its own sub-state with its own
reducers/selectors (see `ui/src/projects/branches.ts`), even when it reuses the
same operand/navigation machinery. Don't multiplex an existing state container
behind mode conditionals — the tell is an `if (tab === ...)` guard, or a
comment explaining a special case, in code that shouldn't know that mode
exists.

# GitButler comments

GitButler supports experimental interactive comment threads between humans and multiple agents.

We use a primary agent to work and send messages and acknowledgements, and a subagent to listen for messages in active threads.

## Primary agent instructions

As the primary agent, when asked to check comments, listen for feedback, or collaborate through GitButler, it is your job to spawn the listener subagent for each project/repo you're working on, listen for messages from them, acknowledge and potentially reply to them, and act on work according to these messages.

For example:

1. Spawn subagent and listen.
2. Receive message "change font size of text" from subagent.
3. Acknowledge receipt of the message.
4. Do your work.
5. Reply again in the relevant thread.
6. Continue listening to the subagent, including in the middle of in-progress work, akin to steering.

Wait for the subagent to report that it is ready before reporting that the listener is running. While a listener is active, check for forwarded messages before responding to the human; agent status alone does not show pending messages.

While a listener is active, remain in the turn and wait for subagent messages.

Because the listener is listening on a blocking command, send replies from the primary agent directly.

### Acknowledgement & reply

Always acknowledge seen messages. You may reply to them at the same time.

```console
$ but _comment reply <thread-id> \
  --client-id pi-feature-foo-uniqueidhere \
  --author Pi \
  --author-kind agent \
  --ack-through <message-id> \
  --message 'I have implemented the change you requested.'
```

```console
$ but _comment ack <thread-id> \
  --client-id pi-feature-foo-uniqueidhere \
  --message <message-id>
```

### Resolution

You may resolve a thread only when the work is confidently complete or the human explicitly requests it.

```console
$ but _comment archive <thread-id>
```

## Listener subagent instructions

These instructions are for the spawned subagent.

Choose one stable client ID, friendly agent name, and short, friendly title for your agentic workstream. Only the client ID needs to be unique. The agent name should be intuitive, for example "Codex" or "Claude". The title ideally mimics your harness thread title verbatim. You will additionally identify yourself as an agent.

As per the but CLI run commands from the project directory.

Before listening, report to the primary agent that you are ready and include your client ID, author, and title.

### Listening

```console
$ but _comment list --wait \
  --client-id pi-feature-foo-uniqueidhere \
  --author Pi \
  --author-kind agent \
  --title 'Implement feature foo'
```

`list --wait` returns after delivering work or timing out.

After each non-timeout delivery, acknowledge through the newest message returned, forward the thread to the primary agent, and then resume listening. Do not reply, that is the responsiblity of the primary agent.

Do not stop listening or stop repeating the command unless explicitly instructed.

# Verifying your work

Always run the specified commands **exactly** as written.

## Typechecking

Typechecking is the fastest way to validate that everything is okay.

```console
$ pnpm -F @gitbutler/lite check
```

## Testing

Our unit tests are written with Vitest and our E2E tests with Playwright.

```console
$ pnpm -F @gitbutler/lite test
$ pnpm -F @gitbutler/lite test:e2e
```

## Linting & formatting

Once the work is functionally complete, run the following linters and formatters.

```console
$ pnpm oxlint:fix
$ pnpm knip:prod
$ pnpm knip:non-prod
$ pnpm exec oxfmt apps/lite
$ pnpm exec prettier --write apps/lite
```
