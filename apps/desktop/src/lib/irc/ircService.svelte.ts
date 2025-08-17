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
	messageNick,
	getChatsWithPopup,
	getChannel,
	setPopup,
	getChat,
	clearNames
} from '$lib/irc/ircSlice';
import { showError } from '$lib/notifications/toasts';
import { InjectionToken } from '@gitbutler/shared/context';
import { reactive } from '@gitbutler/shared/reactiveUtils.svelte';
import persistReducer from 'redux-persist/es/persistReducer';
import storage from 'redux-persist/lib/storage';
import type { IrcClient } from '$lib/irc/ircClient.svelte';
import type { IrcEvent } from '$lib/irc/parser';
import type { IrcChannel, IrcChat, WhoInfo } from '$lib/irc/types';
import type { ClientState } from '$lib/state/clientState.svelte';
import type { Reactive } from '@gitbutler/shared/storeUtils';
import type { ThunkDispatch, UnknownAction } from '@reduxjs/toolkit';

export const IRC_SERVICE = new InjectionToken<IrcService>('IrcService');

/**
 * Experimental IRC
 */
export class IrcService {
	private state = $state.raw(ircSlice.getInitialState());
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
				if (ircSlice.reducerPath in clientState.reactiveState) {
					// @ts-expect-error code-splitting means it's not defined in client state.
					this.state = clientState.reactiveState[ircSlice.reducerPath] as IRCState;
				}
			}
		});

		$effect(() => {
			return this.ircClient.onevent(async (event) => {
				return this.handleEvent(event);
			});
		});

		$effect(() => {
			return this.ircClient.onopen(() => {
				const channels = this.getChannels();
				this.dispatch(clearNames());
				setTimeout(() => {
					for (const channel of channels.current) {
						this.send(`JOIN ${channel?.name}`);
					}
				}, 5000);
			});
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
		return selectSystemMessages(this.state);
	}

	getChannelMessages(channel: string) {
		return selectChannelMessages(this.state, channel);
	}

	getPrivateMessages(nick: string) {
		return selectPrivateMessages(this.state, nick);
	}

	getChats() {
		const result = $derived(getChats(this.state));
		return reactive(() => result);
	}

	getChatsWithPopup() {
		const result = $derived(getChatsWithPopup(this.state));
		return reactive(() => result);
	}

	getChannels() {
		const result = $derived(getChannels(this.state));
		return reactive(() => result);
	}

	getChannel(name: string): Reactive<IrcChannel | undefined> {
		const selector = $derived(getChannel(name));
		const result = $derived(selector(this.state));
		return reactive(() => result);
	}

	getChat(name: string): Reactive<IrcChat | undefined> {
		const selector = $derived(getChat(name));
		const result = $derived(selector(this.state));
		return reactive(() => result);
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

	setPopup(name: string, popup: boolean) {
		this.dispatch(setPopup({ name, floating: popup }));
	}

	unreadCount() {
		const result = $derived(getUnreadCount(this.state));
		return reactive(() => result);
	}
}
