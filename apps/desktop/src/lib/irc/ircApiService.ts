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

import { InjectionToken } from "@gitbutler/core/context";
import type { BackendApi } from "$lib/state/clientState.svelte";

export const IRC_API_SERVICE = new InjectionToken<IrcApiService>("IrcApiService");

export const IRC_CONNECTION_ID = "personal-irc";

export class IrcApiService {
	constructor(private backendApi: BackendApi) {}

	// ── Queries ──────────────────────────────────────────────────────

	connectionState() {
		return this.backendApi.endpoints.ircGetConnectionState.useQuery({ id: IRC_CONNECTION_ID });
	}

	async fetchConnectionState() {
		return await this.backendApi.endpoints.ircGetConnectionState.fetch({ id: IRC_CONNECTION_ID });
	}

	channels() {
		return this.backendApi.endpoints.ircGetChannels.useQuery({ id: IRC_CONNECTION_ID });
	}

	allUsers() {
		return this.backendApi.endpoints.ircGetChannels.useQuery(
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
		return await this.backendApi.endpoints.ircGetChannels.fetch({ id: IRC_CONNECTION_ID });
	}

	messages(args: { channel: string }) {
		return this.backendApi.endpoints.ircGetMessages.useQuery({ ...args, id: IRC_CONNECTION_ID });
	}

	async fetchMessages(args: { channel: string }) {
		return await this.backendApi.endpoints.ircGetMessages.fetch({ ...args, id: IRC_CONNECTION_ID });
	}

	users(args: { channel: string }) {
		return this.backendApi.endpoints.ircGetUsers.useQuery({ ...args, id: IRC_CONNECTION_ID });
	}

	async fetchUsers(args: { channel: string }) {
		return await this.backendApi.endpoints.ircGetUsers.fetch({ ...args, id: IRC_CONNECTION_ID });
	}

	nick() {
		return this.backendApi.endpoints.ircGetNick.useQuery({ id: IRC_CONNECTION_ID });
	}

	async fetchNick() {
		return await this.backendApi.endpoints.ircGetNick.fetch({ id: IRC_CONNECTION_ID });
	}

	commitReactions() {
		return this.backendApi.endpoints.ircGetAllReactions.useQuery({ id: IRC_CONNECTION_ID });
	}

	async fetchCommitReactions() {
		return await this.backendApi.endpoints.ircGetAllReactions.fetch({ id: IRC_CONNECTION_ID });
	}

	messageReactions() {
		return this.backendApi.endpoints.ircGetAllMessageReactions.useQuery({ id: IRC_CONNECTION_ID });
	}

	async fetchMessageReactions() {
		return await this.backendApi.endpoints.ircGetAllMessageReactions.fetch({
			id: IRC_CONNECTION_ID,
		});
	}

	fileMessageReactions(args: { filePath: string }) {
		return this.backendApi.endpoints.ircGetFileMessageReactions.useQuery({
			...args,
			id: IRC_CONNECTION_ID,
		});
	}

	async fetchFileMessageReactions(args: { filePath: string }) {
		return await this.backendApi.endpoints.ircGetFileMessageReactions.fetch({
			...args,
			id: IRC_CONNECTION_ID,
		});
	}

	workingFiles(args: { channel: string }) {
		return this.backendApi.endpoints.ircGetWorkingFiles.useQuery({
			...args,
			id: IRC_CONNECTION_ID,
		});
	}

	// ── Mutations ────────────────────────────────────────────────────

	async sendMessage(args: { target: string; message: string; replyTo?: string }) {
		return await this.backendApi.endpoints.ircSendMessage.mutate({
			...args,
			id: IRC_CONNECTION_ID,
		});
	}

	async sendMessageWithData(args: {
		target: string;
		message: string;
		data: string;
		replyTo?: string;
	}) {
		return await this.backendApi.endpoints.ircSendMessageWithData.mutate({
			...args,
			id: IRC_CONNECTION_ID,
		});
	}

	async sendRaw(args: { command: string }) {
		return await this.backendApi.endpoints.ircSendRaw.mutate({ ...args, id: IRC_CONNECTION_ID });
	}

	async disconnect() {
		return await this.backendApi.endpoints.ircDisconnect.mutate({ id: IRC_CONNECTION_ID });
	}

	async joinChannel(args: { channel: string }) {
		return await this.backendApi.endpoints.ircJoinChannel.mutate({
			...args,
			id: IRC_CONNECTION_ID,
		});
	}

	async partChannel(args: { channel: string }) {
		return await this.backendApi.endpoints.ircPartChannel.mutate({
			...args,
			id: IRC_CONNECTION_ID,
		});
	}

	async autoJoin(args: { channel: string }) {
		return await this.backendApi.endpoints.ircAutoJoin.mutate({ ...args, id: IRC_CONNECTION_ID });
	}

	async autoLeave(args: { channel: string }) {
		return await this.backendApi.endpoints.ircAutoLeave.mutate({ ...args, id: IRC_CONNECTION_ID });
	}

	async markRead(args: { channel: string }) {
		return await this.backendApi.endpoints.ircMarkRead.mutate({ ...args, id: IRC_CONNECTION_ID });
	}

	async clearMessages(args: { channel: string }) {
		return await this.backendApi.endpoints.ircClearMessages.mutate({
			...args,
			id: IRC_CONNECTION_ID,
		});
	}

	async requestHistoryBefore(args: { channel: string; before: string; limit?: number }) {
		return await this.backendApi.endpoints.ircRequestHistoryBefore.mutate({
			...args,
			id: IRC_CONNECTION_ID,
		});
	}

	async sendTyping(args: { target: string; state: string }) {
		return await this.backendApi.endpoints.ircSendTyping.mutate({ ...args, id: IRC_CONNECTION_ID });
	}

	async sendReaction(args: { target: string; msgid: string; emoji: string }) {
		return await this.backendApi.endpoints.ircSendReaction.mutate({
			...args,
			id: IRC_CONNECTION_ID,
		});
	}

	async removeReaction(args: { target: string; msgid: string; emoji: string }) {
		return await this.backendApi.endpoints.ircRemoveReaction.mutate({
			...args,
			id: IRC_CONNECTION_ID,
		});
	}

	async redactMessage(args: { target: string; msgid: string; reason?: string }) {
		return await this.backendApi.endpoints.ircRedactMessage.mutate({
			...args,
			id: IRC_CONNECTION_ID,
		});
	}
}
