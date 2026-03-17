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

	async fetchConnectionState() {
		return await this.api.endpoints.ircGetConnectionState.fetch({ id: IRC_CONNECTION_ID });
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
							const nick = u.nick.replace(/^[@+%~&]/, "");
							allUsers.add(nick);
						}
					}
					const users = Array.from(allUsers.values());
					users.sort();
					return users;
				},
			},
		);
	}

	async fetchChannels() {
		return await this.api.endpoints.ircGetChannels.fetch({ id: IRC_CONNECTION_ID });
	}

	messages(args: { channel: string }) {
		return this.api.endpoints.ircGetMessages.useQuery({ ...args, id: IRC_CONNECTION_ID });
	}

	async fetchMessages(args: { channel: string }) {
		return await this.api.endpoints.ircGetMessages.fetch({ ...args, id: IRC_CONNECTION_ID });
	}

	users(args: { channel: string }) {
		return this.api.endpoints.ircGetUsers.useQuery({ ...args, id: IRC_CONNECTION_ID });
	}

	async fetchUsers(args: { channel: string }) {
		return await this.api.endpoints.ircGetUsers.fetch({ ...args, id: IRC_CONNECTION_ID });
	}

	nick() {
		return this.api.endpoints.ircGetNick.useQuery({ id: IRC_CONNECTION_ID });
	}

	async fetchNick() {
		return await this.api.endpoints.ircGetNick.fetch({ id: IRC_CONNECTION_ID });
	}

	commitReactions() {
		return this.api.endpoints.ircGetAllReactions.useQuery({ id: IRC_CONNECTION_ID });
	}

	async fetchCommitReactions() {
		return await this.api.endpoints.ircGetAllReactions.fetch({ id: IRC_CONNECTION_ID });
	}

	messageReactions() {
		return this.api.endpoints.ircGetAllMessageReactions.useQuery({ id: IRC_CONNECTION_ID });
	}

	async fetchMessageReactions() {
		return await this.api.endpoints.ircGetAllMessageReactions.fetch({ id: IRC_CONNECTION_ID });
	}

	fileMessageReactions(args: { filePath: string }) {
		return this.api.endpoints.ircGetFileMessageReactions.useQuery({
			...args,
			id: IRC_CONNECTION_ID,
		});
	}

	async fetchFileMessageReactions(args: { filePath: string }) {
		return await this.api.endpoints.ircGetFileMessageReactions.fetch({
			...args,
			id: IRC_CONNECTION_ID,
		});
	}

	workingFiles(args: { channel: string }) {
		return this.api.endpoints.ircGetWorkingFiles.useQuery({ ...args, id: IRC_CONNECTION_ID });
	}

	// ── Mutations ────────────────────────────────────────────────────

	async sendMessage(args: { target: string; message: string; replyTo?: string }) {
		return await this.api.endpoints.ircSendMessage.mutate({ ...args, id: IRC_CONNECTION_ID });
	}

	async sendMessageWithData(args: {
		target: string;
		message: string;
		data: string;
		replyTo?: string;
	}) {
		return await this.api.endpoints.ircSendMessageWithData.mutate({
			...args,
			id: IRC_CONNECTION_ID,
		});
	}

	async sendRaw(args: { command: string }) {
		return await this.api.endpoints.ircSendRaw.mutate({ ...args, id: IRC_CONNECTION_ID });
	}

	async disconnect() {
		return await this.api.endpoints.ircDisconnect.mutate({ id: IRC_CONNECTION_ID });
	}

	async joinChannel(args: { channel: string }) {
		return await this.api.endpoints.ircJoinChannel.mutate({ ...args, id: IRC_CONNECTION_ID });
	}

	async partChannel(args: { channel: string }) {
		return await this.api.endpoints.ircPartChannel.mutate({ ...args, id: IRC_CONNECTION_ID });
	}

	async autoJoin(args: { channel: string }) {
		return await this.api.endpoints.ircAutoJoin.mutate({ ...args, id: IRC_CONNECTION_ID });
	}

	async autoLeave(args: { channel: string }) {
		return await this.api.endpoints.ircAutoLeave.mutate({ ...args, id: IRC_CONNECTION_ID });
	}

	async markRead(args: { channel: string }) {
		return await this.api.endpoints.ircMarkRead.mutate({ ...args, id: IRC_CONNECTION_ID });
	}

	async clearMessages(args: { channel: string }) {
		return await this.api.endpoints.ircClearMessages.mutate({ ...args, id: IRC_CONNECTION_ID });
	}

	async requestHistoryBefore(args: { channel: string; before: string; limit?: number }) {
		return await this.api.endpoints.ircRequestHistoryBefore.mutate({
			...args,
			id: IRC_CONNECTION_ID,
		});
	}

	async sendTyping(args: { target: string; state: string }) {
		return await this.api.endpoints.ircSendTyping.mutate({ ...args, id: IRC_CONNECTION_ID });
	}

	async sendReaction(args: { target: string; msgid: string; emoji: string }) {
		return await this.api.endpoints.ircSendReaction.mutate({ ...args, id: IRC_CONNECTION_ID });
	}

	async removeReaction(args: { target: string; msgid: string; emoji: string }) {
		return await this.api.endpoints.ircRemoveReaction.mutate({ ...args, id: IRC_CONNECTION_ID });
	}

	async redactMessage(args: { target: string; msgid: string; reason?: string }) {
		return await this.api.endpoints.ircRedactMessage.mutate({ ...args, id: IRC_CONNECTION_ID });
	}
}
