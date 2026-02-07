/**
 * IRC RTKQ API — queries and mutations backed by Rust Tauri commands.
 *
 * This replaces the ircSlice + IrcService pattern for data fetching.
 * Data lives in the backend MessageStore; the frontend queries it and
 * subscribes to granular WebSocket events to keep the cache fresh.
 */

import { hasBackendExtra } from "$lib/state/backendQuery";
import { invalidatesItem, providesItem, ReduxTag } from "$lib/state/tags";
import type { BackendApi } from "$lib/state/clientState.svelte";

// ---------------------------------------------------------------------------
// Types matching Rust backend structs (camelCase via serde rename_all)
// ---------------------------------------------------------------------------

export type MessageDirection = "incoming" | "outgoing";

export interface UserEntry {
	nick: string;
	away: boolean;
}

export interface CommitReaction {
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
	msgid: string;
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

// ---------------------------------------------------------------------------
// RTKQ endpoint injection
// ---------------------------------------------------------------------------

export function injectIrcEndpoints(api: BackendApi) {
	return api.injectEndpoints({
		endpoints: (build) => ({
			// ── Queries ──────────────────────────────────────────────

			/** Get the connection state for an IRC connection. */
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

					// Re-fetch whenever backend emits a state change event
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

			/** Get channels (with metadata) for an IRC connection. */
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

					// Channel list changes when users join/part or topics change
					const unsub1 = listen(`irc:${arg.id}:channels`, async () => {
						const channels = await invoke<ChannelInfo[]>("irc_channels", {
							id: arg.id,
						});
						lifecycleApi.updateCachedData(() => channels);
					});

					// User join/part also affects channel metadata (user counts)
					const unsub2 = listen(`irc:${arg.id}:users`, async () => {
						const channels = await invoke<ChannelInfo[]>("irc_channels", {
							id: arg.id,
						});
						lifecycleApi.updateCachedData(() => channels);
					});

					// New messages affect unread counts
					const unsub3 = listen(`irc:${arg.id}:message`, async () => {
						const channels = await invoke<ChannelInfo[]>("irc_channels", {
							id: arg.id,
						});
						lifecycleApi.updateCachedData(() => channels);
					});

					await lifecycleApi.cacheEntryRemoved;
					unsub1();
					unsub2();
					unsub3();
				},
			}),

			/** Get stored messages for a specific channel/target. */
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

					// Insert messages in sorted order as they arrive
					const unsub1 = listen<StoredMessage>(`irc:${arg.id}:message`, async (event) => {
						const msg = event.payload;
						// Only add messages for our channel
						if (msg.target === arg.channel) {
							lifecycleApi.updateCachedData((messages) => {
								// Deduplicate by msgid
								if (msg.msgid && messages.some((m) => m.msgid === msg.msgid)) {
									return;
								}
								// Binary search for insertion point by timestamp
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

					// Merge an entire history batch into the cache at once
					const unsub2 = listen<{ channel: string; messages: StoredMessage[] }>(
						`irc:${arg.id}:history-batch`,
						async (event) => {
							if (event.payload.channel === arg.channel) {
								lifecycleApi.updateCachedData((cached) => {
									const batch = event.payload.messages;
									if (batch.length === 0) return;

									// Batch is sorted ascending by timestamp. Find insertion
									// point for the first message and splice the whole chunk in.
									const firstTs = batch[0]!.timestamp;
									let lo = 0;
									let hi = cached.length;
									while (lo < hi) {
										const mid = (lo + hi) >>> 1;
										if (cached[mid]!.timestamp <= firstTs) {
											lo = mid + 1;
										} else {
											hi = mid;
										}
									}
									cached.splice(lo, 0, ...batch);
								});
							}
						},
					);

					await lifecycleApi.cacheEntryRemoved;
					unsub1();
					unsub2();
				},
			}),

			/** Get the user list for a channel. */
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

			/** Get the nickname for an IRC connection. */
			ircGetNick: build.query<string, { id: string }>({
				extraOptions: { command: "irc_nick" },
				query: (args) => args,
				providesTags: (_result, _error, args) => [
					...providesItem(ReduxTag.IrcConnectionState, "nick:" + args.id),
				],
			}),

			// ── Mutations ────────────────────────────────────────────

			/** Send a message to a channel/target. */
			ircSendMessage: build.mutation<
				undefined,
				{ id: string; target: string; message: string; replyTo?: string }
			>({
				extraOptions: {
					command: "irc_send_message",
					actionName: "Send IRC message",
				},
				query: (args) => args,
			}),

			/** Get all commit reactions for a connection, keyed by commitId. */
			ircGetAllCommitReactions: build.query<Record<string, CommitReaction[]>, { id: string }>({
				extraOptions: { command: "irc_get_all_commit_reactions" },
				query: (args) => args,
				async onCacheEntryAdded(arg, lifecycleApi) {
					if (!hasBackendExtra(lifecycleApi.extra)) {
						throw new Error("Redux dependency Backend not found!");
					}
					const { listen, invoke } = lifecycleApi.extra.backend;
					await lifecycleApi.cacheDataLoaded;

					const unsubscribe = listen(`irc:${arg.id}:commit-reaction`, async () => {
						const reactions = await invoke<Record<string, CommitReaction[]>>(
							"irc_get_all_commit_reactions",
							{ id: arg.id },
						);
						lifecycleApi.updateCachedData(() => reactions);
					});
					await lifecycleApi.cacheEntryRemoved;
					unsubscribe();
				},
			}),

			/** Get all message reactions for a connection, keyed by message ID. */
			ircGetAllMessageReactions: build.query<Record<string, CommitReaction[]>, { id: string }>({
				extraOptions: { command: "irc_get_all_message_reactions" },
				query: (args) => args,
				async onCacheEntryAdded(arg, lifecycleApi) {
					if (!hasBackendExtra(lifecycleApi.extra)) {
						throw new Error("Redux dependency Backend not found!");
					}
					const { listen, invoke } = lifecycleApi.extra.backend;
					await lifecycleApi.cacheDataLoaded;

					const unsubscribe = listen(`irc:${arg.id}:message-reaction`, async () => {
						const reactions = await invoke<Record<string, CommitReaction[]>>(
							"irc_get_all_message_reactions",
							{ id: arg.id },
						);
						lifecycleApi.updateCachedData(() => reactions);
					});
					await lifecycleApi.cacheEntryRemoved;
					unsubscribe();
				},
			}),

			/** Get message reactions for a specific file, keyed by hunk key (oldStart:oldLines:newStart:newLines). */
			ircGetFileMessageReactions: build.query<
				Record<string, CommitReaction[]>,
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
						const reactions = await invoke<Record<string, CommitReaction[]>>(
							"irc_get_file_message_reactions",
							{ id: arg.id, filePath: arg.filePath },
						);
						lifecycleApi.updateCachedData(() => reactions);
					});
					await lifecycleApi.cacheEntryRemoved;
					unsubscribe();
				},
			}),

			/** Get the current working files for all users in a channel (nick → file paths). */
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

			/** Send a message with a data payload. */
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

			/** Disconnect an IRC connection. */
			ircDisconnect: build.mutation<undefined, { id: string }>({
				extraOptions: {
					command: "irc_disconnect",
					actionName: "Disconnect IRC",
				},
				query: (args) => args,
				invalidatesTags: (_result, _error, args) => [
					invalidatesItem(ReduxTag.IrcConnectionState, args.id),
				],
			}),

			/** Join a channel. */
			ircJoinChannel: build.mutation<undefined, { id: string; channel: string }>({
				extraOptions: {
					command: "irc_join",
					actionName: "Join IRC channel",
				},
				query: (args) => args,
				invalidatesTags: (_result, _error, args) => [
					invalidatesItem(ReduxTag.IrcChannels, args.id),
				],
			}),

			/** Part (leave) a channel. */
			ircPartChannel: build.mutation<undefined, { id: string; channel: string }>({
				extraOptions: {
					command: "irc_part",
					actionName: "Part IRC channel",
				},
				query: (args) => args,
				invalidatesTags: (_result, _error, args) => [
					invalidatesItem(ReduxTag.IrcChannels, args.id),
				],
			}),

			/** Add a channel to the auto-join set (joins immediately if connected, re-joins on reconnect). */
			ircAutoJoin: build.mutation<undefined, { id: string; channel: string }>({
				extraOptions: {
					command: "irc_auto_join",
					actionName: "Auto-join IRC channel",
				},
				query: (args) => args,
				invalidatesTags: (_result, _error, args) => [
					invalidatesItem(ReduxTag.IrcChannels, args.id),
				],
			}),

			/** Remove a channel from the auto-join set and part it. */
			ircAutoLeave: build.mutation<undefined, { id: string; channel: string }>({
				extraOptions: {
					command: "irc_auto_leave",
					actionName: "Auto-leave IRC channel",
				},
				query: (args) => args,
				invalidatesTags: (_result, _error, args) => [
					invalidatesItem(ReduxTag.IrcChannels, args.id),
				],
			}),

			/** Mark a channel as read. */
			ircMarkRead: build.mutation<undefined, { id: string; channel: string }>({
				extraOptions: {
					command: "irc_mark_read",
					actionName: "Mark IRC channel read",
				},
				query: (args) => args,
				invalidatesTags: (_result, _error, args) => [
					invalidatesItem(ReduxTag.IrcChannels, args.id),
				],
			}),

			/** Clear stored messages for a channel. */
			ircClearMessages: build.mutation<undefined, { id: string; channel: string }>({
				extraOptions: {
					command: "irc_clear_messages",
					actionName: "Clear IRC messages",
				},
				query: (args) => args,
				invalidatesTags: (_result, _error, args) => [
					invalidatesItem(ReduxTag.IrcMessages, args.id + ":" + args.channel),
				],
			}),

			/** Send a raw IRC command (e.g. "JOIN #channel" or "NAMES #channel"). */
			ircSendRaw: build.mutation<undefined, { id: string; command: string }>({
				extraOptions: {
					command: "irc_send_raw",
					actionName: "Send raw IRC command",
				},
				query: (args) => args,
			}),

			/** Request older chat history before a timestamp. */
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
		}),
	});
}
