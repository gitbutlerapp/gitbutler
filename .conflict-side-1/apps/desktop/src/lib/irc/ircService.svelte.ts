import {
	getUnreadCount,
	ircSlice,
	messageChannel,
	processIncoming,
	selectChannelMessages,
	getChannels,
	selectSystemMessages,
	setNick,
	setUser,
	getChannelUsers,
	selectPrivateMessages,
	getChats,
	markOpen,
	messageNick
} from '$lib/irc/ircSlice';
import { showError } from '$lib/notifications/toasts';
import { reactive } from '@gitbutler/shared/reactiveUtils.svelte';
import persistReducer from 'redux-persist/es/persistReducer';
import storage from 'redux-persist/lib/storage';
import type { IrcClient, ReadyState } from '$lib/irc/ircClient.svelte';
import type { IrcEvent } from '$lib/irc/parser';
import type { WhoInfo } from '$lib/irc/types';
import type { ClientState } from '$lib/state/clientState.svelte';
import type { ThunkDispatch, UnknownAction } from '@reduxjs/toolkit';

/**
 * Experimental IRC
 */
export class IrcService {
	private state = $state(ircSlice.getInitialState());
	_whoInfo: WhoInfo | undefined;
	status: ReadyState | undefined = $state();

	constructor(
		clientState: ClientState,
		private dispatch: ThunkDispatch<any, any, UnknownAction>,
		private ircClient: IrcClient
	) {
		const persistConfig = {
			key: ircSlice.reducerPath,
			storage: storage
		};

		clientState.inject(ircSlice.reducerPath, persistReducer(persistConfig, ircSlice.reducer));

		$effect(() => {
			if (clientState.reactiveState) {
				// @ts-expect-error code-splitting means it's not defined in client state.
				this.state = clientState.reactiveState[ircSlice.reducerPath] as IRCState;
			}
		});

		$effect(() => {
			return this.ircClient.onevent(async (event) => {
				return this.handleEvent(event);
			});
		});

		$effect(() => {
			// return this.ircClient.onopen(() => {
			// 	const channels = this.getChannels();
			// 	this.dispatch(clearNames());
			// 	setTimeout(() => {
			// 		for (const key in channels) {
			// 			const channel = channels[key];
			// 			this.send(`JOIN ${channel?.name}`);
			// 		}
			// 	}, 5000);
			// });
		});
	}

	get whoInfo() {
		return reactive(() => this._whoInfo);
	}

	async setWhoInfo(args: WhoInfo) {
		this._whoInfo = args;
		await this.dispatch(setNick(args.nick));
		await this.dispatch(
			setUser({
				username: args.username,
				realname: args.realname
			})
		);
	}

	private handleEvent(event: IrcEvent) {
		try {
			this.dispatch(processIncoming(event));
		} catch (err: unknown) {
			showError('IRC error', err);
		}
	}

	// Sending NAMES request when joining a channel
	sendNamesRequest(channel: string) {
		this.ircClient.send(`NAMES ${channel}\r\n`);
	}

	send(message: string) {
		this.ircClient.send(message);
	}

	async sendToGroup(channel: string, message: string, data?: unknown) {
		return await this.dispatch(
			messageChannel({
				channel,
				message,
				data
			})
		);
	}

	async sendToNick(nick: string, message: string, data?: string | undefined) {
		return await this.dispatch(
			messageNick({
				nick,
				message,
				data
			})
		);
	}

	disconnect() {
		this.ircClient.disconnect();
	}

	getServerMessages() {
		const result = $derived(selectSystemMessages(this.state));
		return result;
	}

	getChannelMessages(channel: string) {
		const result = $derived(selectChannelMessages(this.state, channel));
		return result;
	}

	getPrivateMessages(nick: string) {
		const result = $derived(selectPrivateMessages(this.state, nick));
		return result;
	}

	getChats() {
		const result = $derived(getChats(this.state));
		return result;
	}

	getChannels() {
		const result = $derived(getChannels(this.state));
		return result;
	}

	getChannelUsers(name: string) {
		const result = $derived(getChannelUsers(this.state, name));
		return reactive(() => result);
	}

	markOpen(name: string) {
		this.dispatch(markOpen({ name, open: true }));
		return () => {
			this.dispatch(markOpen({ name, open: false }));
		};
	}

	unreadCount() {
		const result = $derived(getUnreadCount(this.state));
		return reactive(() => result);
	}
}
