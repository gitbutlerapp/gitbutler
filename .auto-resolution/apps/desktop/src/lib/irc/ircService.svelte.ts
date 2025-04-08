import {
	ircSlice,
	messageChannel,
	processIncoming,
	selectChannelMessages,
	selectChannels,
	selectSystemMessages,
	setNick,
	setUser
} from '$lib/irc/ircSlice';
import { showError } from '$lib/notifications/toasts';
import persistReducer from 'redux-persist/es/persistReducer';
import storage from 'redux-persist/lib/storage';
import type { IrcClient, ReadyState } from '$lib/irc/ircClient.svelte';
import type { WhoInfo } from '$lib/irc/types';
import type { ClientState } from '$lib/state/clientState.svelte';
import type { ThunkDispatch, UnknownAction } from '@reduxjs/toolkit';

/**
 * A service for tracking uncommitted changes.
 *
 * Since we want to maintain a list and access individual records we use a
 * redux entity adapter on the results.
 */
export class IrcService {
	state = $state(ircSlice.getInitialState());
	whoInfo: WhoInfo | undefined;
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
			return this.ircClient.onevent(async (message) => await this.handleMessage(message));
		});
		$effect(() => {
			return this.ircClient.onopen(() => {
				const channels = this.getChannels();
				setTimeout(() => {
					for (const key in channels) {
						const channel = channels[key];
						this.send(`JOIN ${channel?.name}`);
					}
				}, 2000);
			});
		});
	}

	async setWhoInfo(args: WhoInfo) {
		await this.dispatch(setNick(args.nick));
		await this.dispatch(
			setUser({
				username: args.username,
				realname: args.realname
			})
		);
	}

	private async handleMessage(message: string) {
		try {
			await this.dispatch(processIncoming(message));
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

	async sendToGroup(message: string, channel: string) {
		return await this.dispatch(
			messageChannel({
				channel,
				message
			})
		);
	}

	disconnect() {
		this.ircClient.disconnect();
	}

	getSystemMessages() {
		const result = $derived(selectSystemMessages(this.state));
		return result;
	}

	getChannelMessages(channel: string) {
		const result = $derived(selectChannelMessages(this.state, channel));
		return result;
	}

	getChannels() {
		const result = $derived(selectChannels(this.state));
		return result;
	}
}
