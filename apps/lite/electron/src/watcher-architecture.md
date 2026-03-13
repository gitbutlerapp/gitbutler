# Watcher Subscription Architecture

_I have the feeling, somebody is watching me_ - Rockwell

This is an overview of the architectural decisions around the project watcher and how subscribing to it works.

## Why subscription state lives in Node, not Rust

### TLDR

I chose to keep the logic around starting and stopping the watcher in Node, alongside with the managing of subscriptions.
We want to keep the Node layer as simple as possible in Lite, but it made sense to have the subscription manager there.

The Node layer knows about the different windows and IPC channels, as well as their state (has a window been destroyed or a channel closed).

This keeps the UI subscription, as well as the watcher startup logic in Rust, as simple as it gets.

### More details

The Rust side exposed by `crates/but-napi/src/lib.rs` is intentionally minimal:

- `watcher_start(project_id, callback)` starts one watcher and returns a handle.
- `WatcherHandle` only tracks ownership of that single watcher.
- Stopping is ownership-based (`stop()` drops the inner Rust handle).

This keeps Rust close to a stateless "start/forward/stop" boundary from the JavaScript point of view. In particular:

- Rust does not need to track per-window subscribers.
- Rust does not need to know Electron concepts like `WebContents` lifecycle.
- Rust avoids app-specific fanout policy (event channels, sender cleanup rules, per-window subscriptions).

All of those policies are managed in `apps/lite/electron/src/watcher.ts`, where Electron runtime state already exists.

## Watcher deduplication (per project)

`WatcherManager` in Node guarantees at most one active Rust watcher per project id.

The deduplication mechanism is:

1. Check `projectWatchers` first. If present, reuse the existing watcher state.
2. If watcher creation is already in progress, reuse the pending promise from `pendingProjectWatchers`.
3. Otherwise, call `watcherStart(...)` once, store its promise in `pendingProjectWatchers`, then move the created handle into `projectWatchers`.
4. Remove pending entry in `.finally(...)` so failed and successful starts both clear the in-flight slot.

This avoids duplicate Rust watchers during concurrent subscriptions to the same project.

## Subscription behavior is intentionally not deduplicated

A watcher may be shared, but subscriptions are not.

Every `subscribeToProject(...)` call creates:

- A fresh `subscriptionId`.
- A fresh `eventChannel`.
- A separate entry in `watcherSubscriptions`.
- A separate membership in both project-level and sender-level subscription sets.

This is intentional because multiple consumers can exist for the same project:

- Different windows for the same project.
- Multiple independent listeners in one window, each with its own channel lifecycle.

The architecture deduplicates the expensive resource (Rust watcher process), not the app-level listeners.

### Reasonable expectations

It _is_ possible to subscribe multiple times to the same project. And each subscription will have its own channel.
So it's expected that **the UI will only ever create one subscription per project**.

## Cleanup and stop behavior

We need to be extra careful with the watcher threads being spun. I made it so that at different steps we check for dead subscriptions and close them if necessary.

### 1) Explicit unsubscribe

`removeSubscription(subscriptionId)`:

- Removes the subscription from `watcherSubscriptions`.
- Removes it from the sender subscription set (`senderSubscriptions`).
- Removes it from the project watcher subscription set.
- If that project now has zero subscriptions, it calls `projectWatcher.handle.stop()` and removes the project watcher from `projectWatchers`.

### 2) Window destroyed

`registerSenderCleanup(sender)` attaches `sender.once("destroyed", ...)`.

When a window is closed:

- `removeSenderSubscriptions(senderId)` iterates all its subscription ids.
- Each subscription is removed via `removeSubscription(...)`.
- This can cascade into stopping project watchers that no longer have subscribers.

### 3) Dead/broken subscriber during event forwarding

In `forwardWatcherEvent(...)`, if a subscription sender is destroyed or `send(...)` throws:

- The subscription id is marked as dead.
- Dead subscriptions are removed after the loop.

This provides lazy self-healing when stale subscriptions are encountered.

### 4) App shutdown

`destroy()` calls `stopAllWatchersForShutdown()`:

- Stops all currently active watcher handles.
- Clears `pendingProjectWatchers`, `projectWatchers`, `watcherSubscriptions`, and `senderSubscriptions`.
- Resets singleton instance.

This is a final safety net so no watcher ownership remains during process shutdown.

## End-to-end ownership model

- Rust watcher lifetime is tied to the Rust `WatcherHandle` ownership.
- Node/Electron owns those handles and decides when to drop/stop them based on subscriber presence.
- Subscriber lifecycle is fully app-driven (window/channel semantics), while Rust remains focused on producing watcher events.
