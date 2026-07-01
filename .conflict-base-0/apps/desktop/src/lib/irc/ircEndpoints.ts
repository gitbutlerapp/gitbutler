/**
 * IRC RTKQ endpoints — queries and mutations backed by Rust Tauri commands.
 *
 * Data lives in the backend MessageStore; the frontend queries it and
 * subscribes to granular WebSocket events to keep the cache fresh.
 */

import { hasBackendExtra } from "$lib/state/backendQuery";
import { invalidatesItem, providesItem, ReduxTag } from "$lib/state/tags";
import type { BackendEndpointBuilder } from "$lib/state/backendApi";

export type MessageDirection = "incoming" | "outgoing";

export interface UserEntry {
	nick: string;
	away: boolean;
}

export interface Reaction {
	sender: string;
	reaction: string;
}

export interface StoredMessage {
	sender: string;
	content: string;
	data: string | null;
	timestamp: number;
	direction: MessageDirection;
	target: string;
	isHistory?: boolean;
	msgid?: string;
	replyTo?: string;
	/** Event tag (e.g. "001", "notice", "join", "part", "quit", "motd", numeric code). */
	tag?: string;
}

export interface ChannelInfo {
	name: string;
	topic: string | null;
	unreadCount: number;
	users: UserEntry[];
}

export interface ConnectionStateResponse {
	id: string;
	state: string;
	ready: boolean;
}

export function buildIrcEndpoints(build: BackendEndpointBuilder) {
	return {
		// ── Queries ──────────────────────────────────────────────

		ircGetConnectionState: build.query<ConnectionStateResponse, { id: string }>({
			extraOptions: { command: "irc_state" },
			query: (args) => args,
			providesTags: (_result, _error, args) => [
				...providesItem(ReduxTag.IrcConnectionState, args.id),
			],
			async onCacheEntryAdded(arg, lifecycleApi) {
				if (!hasBackendExtra(lifecycleApi.extra)) {
					throw new Error("Redux dependency Backend not found!");
				}
				const { listen, invoke } = lifecycleApi.extra.backend;
				await lifecycleApi.cacheDataLoaded;

				const unsubscribe = listen<string>(`irc:${arg.id}:state`, async () => {
					const state = await invoke<ConnectionStateResponse>("irc_state", {
						id: arg.id,
					});
					lifecycleApi.updateCachedData(() => state);
				});
				await lifecycleApi.cacheEntryRemoved;
				unsubscribe();
			},
		}),

		ircGetChannels: build.query<ChannelInfo[], { id: string }>({
			extraOptions: { command: "irc_channels" },
			query: (args) => args,
			providesTags: (_result, _error, args) => [...providesItem(ReduxTag.IrcChannels, args.id)],
			async onCacheEntryAdded(arg, lifecycleApi) {
				if (!hasBackendExtra(lifecycleApi.extra)) {
					throw new Error("Redux dependency Backend not found!");
				}
				const { listen, invoke } = lifecycleApi.extra.backend;
				await lifecycleApi.cacheDataLoaded;

				const unsub1 = listen(`irc:${arg.id}:channels`, async () => {
					const channels = await invoke<ChannelInfo[]>("irc_channels", {
						id: arg.id,
					});
					lifecycleApi.updateCachedData(() => channels);
				});

				const unsub2 = listen(`irc:${arg.id}:users`, async () => {
					const channels = await invoke<ChannelInfo[]>("irc_channels", {
						id: arg.id,
					});
					lifecycleApi.updateCachedData(() => channels);
				});

				const unsub3 = listen(`irc:${arg.id}:message`, async () => {
					const channels = await invoke<ChannelInfo[]>("irc_channels", {
						id: arg.id,
					});
					lifecycleApi.updateCachedData(() => channels);
				});

				const unsub4 = listen(`irc:${arg.id}:invited`, async () => {
					const channels = await invoke<ChannelInfo[]>("irc_channels", {
						id: arg.id,
					});
					lifecycleApi.updateCachedData(() => channels);
				});

				await lifecycleApi.cacheEntryRemoved;
				unsub1();
				unsub2();
				unsub3();
				unsub4();
			},
		}),

		ircGetMessages: build.query<StoredMessage[], { id: string; channel: string }>({
			extraOptions: { command: "irc_messages" },
			query: (args) => args,
			providesTags: (_result, _error, args) => [
				...providesItem(ReduxTag.IrcMessages, args.id + ":" + args.channel),
			],
			async onCacheEntryAdded(arg, lifecycleApi) {
				if (!hasBackendExtra(lifecycleApi.extra)) {
					throw new Error("Redux dependency Backend not found!");
				}
				const { listen } = lifecycleApi.extra.backend;
				await lifecycleApi.cacheDataLoaded;

				const unsub1 = listen<StoredMessage>(`irc:${arg.id}:message`, async (event) => {
					const msg = event.payload;
					if (msg.target === arg.channel) {
						lifecycleApi.updateCachedData((messages) => {
							if (msg.msgid && messages.some((m) => m.msgid === msg.msgid)) {
								return;
							}
							let lo = 0;
							let hi = messages.length;
							while (lo < hi) {
								const mid = (lo + hi) >>> 1;
								if (messages[mid]!.timestamp <= msg.timestamp) {
									lo = mid + 1;
								} else {
									hi = mid;
								}
							}
							messages.splice(lo, 0, msg);
						});
					}
				});

				const unsub2 = listen<{ channel: string; messages: StoredMessage[] }>(
					`irc:${arg.id}:history-batch`,
					async (event) => {
						if (event.payload.channel === arg.channel) {
							lifecycleApi.updateCachedData((cached) => {
								const batch = event.payload.messages;
								if (batch.length === 0) return;

								const existingIds = new Set(cached.filter((m) => m.msgid).map((m) => m.msgid));

								for (const msg of batch) {
									if (msg.msgid && existingIds.has(msg.msgid)) continue;

									let lo = 0;
									let hi = cached.length;
									while (lo < hi) {
										const mid = (lo + hi) >>> 1;
										if (cached[mid]!.timestamp <= msg.timestamp) {
											lo = mid + 1;
										} else {
											hi = mid;
										}
									}
									cached.splice(lo, 0, msg);
									if (msg.msgid) existingIds.add(msg.msgid);
								}
							});
						}
					},
				);

				const unsub3 = listen<{ target: string; msgid: string; from: string }>(
					`irc:${arg.id}:message-redacted`,
					async (event) => {
						const { target, msgid } = event.payload;
						if (target === arg.channel) {
							lifecycleApi.updateCachedData((messages) => {
								const idx = messages.findIndex((m) => m.msgid === msgid);
								if (idx !== -1) {
									messages.splice(idx, 1);
								}
							});
						}
					},
				);

				await lifecycleApi.cacheEntryRemoved;
				unsub1();
				unsub2();
				unsub3();
			},
		}),

		ircGetUsers: build.query<UserEntry[], { id: string; channel: string }>({
			extraOptions: { command: "irc_users" },
			query: (args) => args,
			providesTags: (_result, _error, args) => [
				...providesItem(ReduxTag.IrcUsers, args.id + ":" + args.channel),
			],
			async onCacheEntryAdded(arg, lifecycleApi) {
				if (!hasBackendExtra(lifecycleApi.extra)) {
					throw new Error("Redux dependency Backend not found!");
				}
				const { listen, invoke } = lifecycleApi.extra.backend;
				await lifecycleApi.cacheDataLoaded;

				const unsubscribe = listen(`irc:${arg.id}:users`, async () => {
					const users = await invoke<UserEntry[]>("irc_users", {
						id: arg.id,
						channel: arg.channel,
					});
					lifecycleApi.updateCachedData(() => users);
				});
				await lifecycleApi.cacheEntryRemoved;
				unsubscribe();
			},
		}),

		ircGetNick: build.query<string, { id: string }>({
			extraOptions: { command: "irc_nick" },
			query: (args) => args,
			providesTags: (_result, _error, args) => [
				...providesItem(ReduxTag.IrcConnectionState, "nick:" + args.id),
			],
		}),

		ircGetAllReactions: build.query<Record<string, Reaction[]>, { id: string }>({
			extraOptions: { command: "irc_get_all_commit_reactions" },
			query: (args) => args,
			async onCacheEntryAdded(arg, lifecycleApi) {
				if (!hasBackendExtra(lifecycleApi.extra)) {
					throw new Error("Redux dependency Backend not found!");
				}
				const { listen, invoke } = lifecycleApi.extra.backend;
				await lifecycleApi.cacheDataLoaded;

				const unsubscribe = listen(`irc:${arg.id}:commit-reaction`, async () => {
					const reactions = await invoke<Record<string, Reaction[]>>(
						"irc_get_all_commit_reactions",
						{ id: arg.id },
					);
					lifecycleApi.updateCachedData(() => reactions);
				});
				await lifecycleApi.cacheEntryRemoved;
				unsubscribe();
			},
		}),

		ircGetAllMessageReactions: build.query<Record<string, Reaction[]>, { id: string }>({
			extraOptions: { command: "irc_get_all_message_reactions" },
			query: (args) => args,
			async onCacheEntryAdded(arg, lifecycleApi) {
				if (!hasBackendExtra(lifecycleApi.extra)) {
					throw new Error("Redux dependency Backend not found!");
				}
				const { listen, invoke } = lifecycleApi.extra.backend;
				await lifecycleApi.cacheDataLoaded;

				const unsubscribe = listen(`irc:${arg.id}:message-reaction`, async () => {
					const reactions = await invoke<Record<string, Reaction[]>>(
						"irc_get_all_message_reactions",
						{ id: arg.id },
					);
					lifecycleApi.updateCachedData(() => reactions);
				});
				await lifecycleApi.cacheEntryRemoved;
				unsubscribe();
			},
		}),

		ircGetFileMessageReactions: build.query<
			Record<string, Reaction[]>,
			{ id: string; filePath: string }
		>({
			extraOptions: { command: "irc_get_file_message_reactions" },
			query: (args) => args,
			async onCacheEntryAdded(arg, lifecycleApi) {
				if (!hasBackendExtra(lifecycleApi.extra)) {
					throw new Error("Redux dependency Backend not found!");
				}
				const { listen, invoke } = lifecycleApi.extra.backend;
				await lifecycleApi.cacheDataLoaded;

				const unsubscribe = listen(`irc:${arg.id}:message-reaction`, async () => {
					const reactions = await invoke<Record<string, Reaction[]>>(
						"irc_get_file_message_reactions",
						{ id: arg.id, filePath: arg.filePath },
					);
					lifecycleApi.updateCachedData(() => reactions);
				});
				await lifecycleApi.cacheEntryRemoved;
				unsubscribe();
			},
		}),

		ircGetWorkingFiles: build.query<Record<string, string[]>, { id: string; channel: string }>({
			extraOptions: { command: "irc_get_working_files" },
			query: (args) => args,
			async onCacheEntryAdded(arg, lifecycleApi) {
				if (!hasBackendExtra(lifecycleApi.extra)) {
					throw new Error("Redux dependency Backend not found!");
				}
				const { listen, invoke } = lifecycleApi.extra.backend;
				await lifecycleApi.cacheDataLoaded;

				const unsubscribe = listen(`irc:${arg.id}:working-files`, async () => {
					const files = await invoke<Record<string, string[]>>("irc_get_working_files", {
						id: arg.id,
						channel: arg.channel,
					});
					lifecycleApi.updateCachedData(() => files);
				});
				await lifecycleApi.cacheEntryRemoved;
				unsubscribe();
			},
		}),

		// ── Mutations ────────────────────────────────────────────

		ircSendMessage: build.mutation<
			undefined,
			{ id: string; target: string; message: string; replyTo?: string }
		>({
			extraOptions: { command: "irc_send_message", actionName: "Send IRC message" },
			query: (args) => args,
		}),

		ircSendMessageWithData: build.mutation<
			undefined,
			{ id: string; target: string; message: string; data: string; replyTo?: string }
		>({
			extraOptions: {
				command: "irc_send_message_with_data",
				actionName: "Send IRC message with data",
			},
			query: (args) => args,
		}),

		ircDisconnect: build.mutation<undefined, { id: string }>({
			extraOptions: { command: "irc_disconnect", actionName: "Disconnect IRC" },
			query: (args) => args,
			invalidatesTags: (_result, _error, args) => [
				invalidatesItem(ReduxTag.IrcConnectionState, args.id),
			],
		}),

		ircJoinChannel: build.mutation<undefined, { id: string; channel: string }>({
			extraOptions: { command: "irc_join", actionName: "Join IRC channel" },
			query: (args) => args,
			invalidatesTags: (_result, _error, args) => [invalidatesItem(ReduxTag.IrcChannels, args.id)],
		}),

		ircPartChannel: build.mutation<undefined, { id: string; channel: string }>({
			extraOptions: { command: "irc_part", actionName: "Part IRC channel" },
			query: (args) => args,
			invalidatesTags: (_result, _error, args) => [invalidatesItem(ReduxTag.IrcChannels, args.id)],
		}),

		ircAutoJoin: build.mutation<undefined, { id: string; channel: string }>({
			extraOptions: { command: "irc_auto_join", actionName: "Auto-join IRC channel" },
			query: (args) => args,
			invalidatesTags: (_result, _error, args) => [invalidatesItem(ReduxTag.IrcChannels, args.id)],
		}),

		ircAutoLeave: build.mutation<undefined, { id: string; channel: string }>({
			extraOptions: { command: "irc_auto_leave", actionName: "Auto-leave IRC channel" },
			query: (args) => args,
			invalidatesTags: (_result, _error, args) => [invalidatesItem(ReduxTag.IrcChannels, args.id)],
		}),

		ircMarkRead: build.mutation<undefined, { id: string; channel: string }>({
			extraOptions: { command: "irc_mark_read", actionName: "Mark IRC channel read" },
			query: (args) => args,
			invalidatesTags: (_result, _error, args) => [invalidatesItem(ReduxTag.IrcChannels, args.id)],
		}),

		ircClearMessages: build.mutation<undefined, { id: string; channel: string }>({
			extraOptions: { command: "irc_clear_messages", actionName: "Clear IRC messages" },
			query: (args) => args,
			invalidatesTags: (_result, _error, args) => [
				invalidatesItem(ReduxTag.IrcMessages, args.id + ":" + args.channel),
			],
		}),

		ircSendRaw: build.mutation<undefined, { id: string; command: string }>({
			extraOptions: { command: "irc_send_raw", actionName: "Send raw IRC command" },
			query: (args) => args,
		}),

		ircSendTyping: build.mutation<undefined, { id: string; target: string; state: string }>({
			extraOptions: { command: "irc_send_typing", actionName: "Send typing indicator" },
			query: (args) => args,
		}),

		ircSendReaction: build.mutation<
			undefined,
			{ id: string; target: string; msgid: string; emoji: string }
		>({
			extraOptions: { command: "irc_send_reaction", actionName: "Send reaction" },
			query: (args) => args,
		}),

		ircRemoveReaction: build.mutation<
			undefined,
			{ id: string; target: string; msgid: string; emoji: string }
		>({
			extraOptions: { command: "irc_remove_reaction", actionName: "Remove reaction" },
			query: (args) => args,
		}),

		ircRedactMessage: build.mutation<
			undefined,
			{ id: string; target: string; msgid: string; reason?: string }
		>({
			extraOptions: { command: "irc_redact_message", actionName: "Redact message" },
			query: (args) => args,
		}),

		ircRequestHistoryBefore: build.mutation<
			undefined,
			{ id: string; channel: string; before: string; limit?: number }
		>({
			extraOptions: {
				command: "irc_request_history_before",
				actionName: "Request older IRC history",
			},
			query: (args) => args,
		}),
	};
}
