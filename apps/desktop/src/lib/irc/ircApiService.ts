/**
 * IRC API Service — provides RTKQ-backed queries and mutations for IRC.
 *
 * Components inject this service to access IRC data from the backend
 * MessageStore. Real-time updates flow via WebSocket event subscriptions
 * wired up in onCacheEntryAdded handlers.
 *
 * The connection ID is injected automatically — callers never need to
 * specify it.
 */

import { injectIrcEndpoints } from "$lib/irc/ircApi";
import { InjectionToken } from "@gitbutler/core/context";
import type { BackendApi } from "$lib/state/clientState.svelte";

export const IRC_API_SERVICE = new InjectionToken<IrcApiService>("IrcApiService");

export const IRC_CONNECTION_ID = "personal-irc";

export class IrcApiService {
	private api: ReturnType<typeof injectIrcEndpoints>;

	constructor(api: BackendApi) {
		this.api = injectIrcEndpoints(api);
	}

	// ── Queries ──────────────────────────────────────────────────────

	connectionState() {
		return this.api.endpoints.ircGetConnectionState.useQuery({ id: IRC_CONNECTION_ID });
	}

	fetchConnectionState() {
		return this.api.endpoints.ircGetConnectionState.fetch({ id: IRC_CONNECTION_ID });
	}

	channels() {
		return this.api.endpoints.ircGetChannels.useQuery({ id: IRC_CONNECTION_ID });
	}

	allUsers() {
		return this.api.endpoints.ircGetChannels.useQuery(
			{ id: IRC_CONNECTION_ID },
			{
				transform: (channels) => {
					if (channels.length === 0) return [];
					const allUsers = new Set<string>();
					for (const ch of channels) {
						if (ch.name === "*") continue;
						for (const u of ch.users) {
							allUsers.add(u.nick.startsWith("@") ? u.nick.slice(1) : u.nick);
						}
					}
					const users = Array.from(allUsers.values());
					users.sort();
					return users;
				},
			},
		);
	}

	fetchChannels() {
		return this.api.endpoints.ircGetChannels.fetch({ id: IRC_CONNECTION_ID });
	}

	messages(args: { channel: string }) {
		return this.api.endpoints.ircGetMessages.useQuery({ ...args, id: IRC_CONNECTION_ID });
	}

	fetchMessages(args: { channel: string }) {
		return this.api.endpoints.ircGetMessages.fetch({ ...args, id: IRC_CONNECTION_ID });
	}

	users(args: { channel: string }) {
		return this.api.endpoints.ircGetUsers.useQuery({ ...args, id: IRC_CONNECTION_ID });
	}

	fetchUsers(args: { channel: string }) {
		return this.api.endpoints.ircGetUsers.fetch({ ...args, id: IRC_CONNECTION_ID });
	}

	nick() {
		return this.api.endpoints.ircGetNick.useQuery({ id: IRC_CONNECTION_ID });
	}

	fetchNick() {
		return this.api.endpoints.ircGetNick.fetch({ id: IRC_CONNECTION_ID });
	}

	commitReactions() {
		return this.api.endpoints.ircGetAllCommitReactions.useQuery({ id: IRC_CONNECTION_ID });
	}

	fetchCommitReactions() {
		return this.api.endpoints.ircGetAllCommitReactions.fetch({ id: IRC_CONNECTION_ID });
	}

	messageReactions() {
		return this.api.endpoints.ircGetAllMessageReactions.useQuery({ id: IRC_CONNECTION_ID });
	}

	fetchMessageReactions() {
		return this.api.endpoints.ircGetAllMessageReactions.fetch({ id: IRC_CONNECTION_ID });
	}

	fileMessageReactions(args: { filePath: string }) {
		return this.api.endpoints.ircGetFileMessageReactions.useQuery({
			...args,
			id: IRC_CONNECTION_ID,
		});
	}

	fetchFileMessageReactions(args: { filePath: string }) {
		return this.api.endpoints.ircGetFileMessageReactions.fetch({
			...args,
			id: IRC_CONNECTION_ID,
		});
	}

	workingFiles(args: { channel: string }) {
		return this.api.endpoints.ircGetWorkingFiles.useQuery({ ...args, id: IRC_CONNECTION_ID });
	}

	// ── Mutations ────────────────────────────────────────────────────

	sendMessage(args: { target: string; message: string; replyTo?: string }) {
		return this.api.endpoints.ircSendMessage.mutate({ ...args, id: IRC_CONNECTION_ID });
	}

	sendMessageWithData(args: { target: string; message: string; data: string; replyTo?: string }) {
		return this.api.endpoints.ircSendMessageWithData.mutate({ ...args, id: IRC_CONNECTION_ID });
	}

	sendRaw(args: { command: string }) {
		return this.api.endpoints.ircSendRaw.mutate({ ...args, id: IRC_CONNECTION_ID });
	}

	disconnect() {
		return this.api.endpoints.ircDisconnect.mutate({ id: IRC_CONNECTION_ID });
	}

	joinChannel(args: { channel: string }) {
		return this.api.endpoints.ircJoinChannel.mutate({ ...args, id: IRC_CONNECTION_ID });
	}

	partChannel(args: { channel: string }) {
		return this.api.endpoints.ircPartChannel.mutate({ ...args, id: IRC_CONNECTION_ID });
	}

	autoJoin(args: { channel: string }) {
		return this.api.endpoints.ircAutoJoin.mutate({ ...args, id: IRC_CONNECTION_ID });
	}

	autoLeave(args: { channel: string }) {
		return this.api.endpoints.ircAutoLeave.mutate({ ...args, id: IRC_CONNECTION_ID });
	}

	markRead(args: { channel: string }) {
		return this.api.endpoints.ircMarkRead.mutate({ ...args, id: IRC_CONNECTION_ID });
	}

	clearMessages(args: { channel: string }) {
		return this.api.endpoints.ircClearMessages.mutate({ ...args, id: IRC_CONNECTION_ID });
	}

	requestHistoryBefore(args: { channel: string; before: string; limit?: number }) {
		return this.api.endpoints.ircRequestHistoryBefore.mutate({
			...args,
			id: IRC_CONNECTION_ID,
		});
	}
}
