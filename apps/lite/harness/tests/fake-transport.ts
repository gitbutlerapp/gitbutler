import type { LiteApiTransport } from "#electron/lite-api.ts";

/**
 * A test transport: handlers stand in for the host's endpoints, `push` for
 * its event emit — the same two primitives every real host supplies.
 */
export type FakeHandlers = Partial<Record<string, (...args: Array<never>) => unknown>>;

export const createFakeTransport = (handlers: FakeHandlers) => {
	const calls: Array<{ channel: string; params: unknown; args: Array<unknown> }> = [];
	const listeners = new Map<string, Set<(payload: unknown) => void>>();

	const transport: LiteApiTransport = {
		invoke: (channel, ...args) => {
			calls.push({ channel, params: args[0], args });
			const handler = handlers[channel];
			// A named rejection turns a missing fixture into an obvious
			// "add a handler" instead of a mystery render failure.
			if (handler === undefined)
				return Promise.reject(new Error(`No fake handler for endpoint: ${channel}`));
			// Every argument, so a variadic endpoint (pathJoin) behaves here as
			// it does over the real transport. Handlers declare `never` params so
			// a fixture of any arity is assignable; calling one needs this cast.
			return Promise.resolve((handler as (...a: Array<unknown>) => unknown)(...args));
		},
		subscribe: (channel, listener) => {
			const set = listeners.get(channel) ?? new Set();
			listeners.set(channel, set);
			set.add(listener);
			return () => {
				set.delete(listener);
			};
		},
		platform: "darwin",
	};

	return {
		transport,
		/** Every invoke in order, for asserting what the panel asked the host. */
		calls,
		/** Deliver an event to whoever subscribed to `channel`. */
		push: (channel: string, payload: unknown): void => {
			for (const listener of listeners.get(channel) ?? []) listener(payload);
		},
		/** Channels with a live listener — how many subscriptions are armed. */
		subscribedChannels: (): Array<string> =>
			[...listeners.entries()].filter(([, set]) => set.size > 0).map(([channel]) => channel),
	};
};

/**
 * The watcher pair every mounted panel calls. Mirrors the host contract:
 * subscribe answers with a fresh event channel; the test pushes events on it.
 */
export const createWatcherHandlers = () => {
	let counter = 0;
	const channels: Array<string> = [];

	return {
		/** Event channels handed out so far, in subscription order. */
		channels,
		handlers: {
			watcherSubscribe: () => {
				counter += 1;
				const eventChannel = `workspace:watcher-event:${counter}`;
				channels.push(eventChannel);
				// Unique across panels, as the host's are: a panel releasing its
				// predecessor's id must not hit its own.
				return { subscriptionId: crypto.randomUUID(), eventChannel };
			},
			watcherUnsubscribe: () => true,
		} satisfies FakeHandlers,
	};
};
