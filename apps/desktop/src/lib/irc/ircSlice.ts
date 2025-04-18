import { parseIRCMessage as parseIrcMessage, toIrcEvent, type IrcEvent } from '$lib/irc/parser';
import {
	createAsyncThunk,
	createSelector,
	createSlice,
	type PayloadAction
} from '@reduxjs/toolkit';
import type { IrcClient } from '$lib/irc/ircClient.svelte';
import type { IRCState, IRCUser, IrcLog, IrcUserInfo } from '$lib/irc/types';

const initialState: IRCState = {
	connection: { connected: false },
	channels: {},
	systemMessages: [],
	whois: {}
};

export const ircSlice = createSlice({
	name: 'irc',
	initialState,
	reducers: {
		setConnectionState(state, action: PayloadAction<boolean>) {
			state.connection.connected = action.payload;
		},
		markChannelOpen(state, action: PayloadAction<{ name: string; open: boolean }>) {
			const channelName = action.payload.name;
			const channel = state.channels[channelName];
			if (channel) {
				channel.open = action.payload.open;
				if (channel.open) {
					channel.unread = 0;
				}
			}
		},
		clearNames(state) {
			Object.keys(state.channels).map((name) => {
				const channel = state.channels[name];
				if (channel) {
					channel.users = {};
				}
			});
		}
	},
	extraReducers: (build) => {
		// Nick change.
		build.addCase(setNick.fulfilled, (state, action) => {
			state.connection.nick = action.payload;
		});
		// Sent a message to a channel.
		build.addCase(messageChannel.fulfilled, (state, action) => {
			const channel = state.channels[action.meta.arg.channel];
			if (channel) {
				channel.logs.push(action.payload);
			}
		});
		// Sending a message to a channel failed.
		build.addCase(messageChannel.rejected, (state, action) => {
			const { channel: name, message } = action.meta.arg;
			let channel = state.channels[name];
			if (!channel) {
				channel = { name, logs: [], users: {}, unread: 0 };
				state.channels[name] = channel;
			}
			if (!channel.logs) {
				channel.logs = [];
			}

			let errorMessage = action.error.message;
			if (action.payload) {
				errorMessage += '\n\n' + action.payload;
			}

			channel.logs.push({
				timestamp: Date.now(),
				type: 'outgoing',
				error: errorMessage,
				to: name,
				from: '', // TODO: What do we do here?
				message
			});
		});
		// Parse incoming message and update state accordingly.
		build.addCase(processIncoming.fulfilled, (state, action) => {
			const event = action.payload;

			switch (event.type) {
				case 'welcome': {
					state.connection = {
						nick: event.message,
						connected: true
					};
					break;
				}
				case 'userJoined': {
					const newLog: IrcLog = {
						type: 'server',
						timestamp: Date.now(),
						message: `${event.user.nick} joined`
					};
					const channel = state.channels[event.channel];
					const nick = event.user.nick;
					if (!channel) {
						state.channels[event.channel] = {
							name: event.channel,
							users: { [nick]: { nick } },
							unread: 0,
							logs: [newLog]
						};
					} else {
						channel.users[nick] = { nick };
						channel.logs.push(newLog);
					}
					break;
				}
				case 'userParted': {
					leaveChannel({ state, nick: event.nick, channelName: event.channel });
					break;
				}
				case 'userQuit': {
					for (const name of Object.keys(state.channels)) {
						leaveChannel({ state, nick: event.nick, channelName: name });
					}
					break;
				}
				case 'namesList': {
					const channel = state.channels[event.channel];

					const users = event.names.map((name) => {
						let mode: IRCUser['mode'];
						let nick = name;

						if (nick.startsWith('@')) {
							mode = 'op';
							nick = nick.slice(1);
						} else if (nick.startsWith('+')) {
							mode = 'voice';
							nick = nick.slice(1);
						}
						return { nick, mode };
					});

					if (channel) {
						channel.users = Object.fromEntries(users.map((u) => [u.nick, u]));
					}
					break;
				}

				case 'messageReceived': {
					const name = event.target;
					const channel = state.channels[name];
					if (channel) {
						if (!channel.open) {
							channel.unread += 1;
						}
						channel.logs.push({
							type: 'incoming',
							timestamp: Date.now(),
							from: event.from,
							message: event.text
						});
						// Trim server output to last 100 messages.
						while (channel.logs.length > 100) {
							channel.logs.shift();
						}
					}
					break;
				}

				case 'channelTopic': {
					const channel = state.channels[event.channel];
					if (channel) channel.topic = event.topic;
					break;
				}

				case 'nickChanged': {
					const whoInfo = state.whois[event.oldNick];
					state.whois[event.oldNick] = undefined;
					state.whois[event.newNick] = whoInfo;
					break;
				}

				case 'whois': {
					state.whois[event.nick] = event;
					break;
				}

				case 'motd':
				case 'serverNotice':
				case 'error': {
					state.systemMessages.push({
						timestamp: Date.now(),
						type: 'server',
						message: event.message
					});
					break;
				}

				case 'unsupported': {
					console.warn('Unsupported command: ' + event.command);
					state.systemMessages.push({
						type: 'server',
						timestamp: Date.now(),
						message: event.raw
					});
					break;
				}
				case 'ping': {
					state.systemMessages.push({
						type: 'server',
						timestamp: Date.now(),
						message: 'PING :' + event.id
					});
					break;
				}
				default: {
					const _: never = event; // Exhaustive list check.
				}
			}

			// Trim server output to last 100 messages.
			while (state.systemMessages.length > 100) {
				state.systemMessages.shift();
			}
		});
	}
});

/** Leave channel used both for PART and QUIT. */
function leaveChannel(args: { state: IRCState; nick: string; channelName: string }) {
	const { state, nick, channelName } = args;
	const channel = state.channels[channelName];
	if (channel) {
		delete channel.users[nick];
		if (Object.keys(channel.users).length === 0) {
			delete state.channels[channelName];
		} else {
			channel.logs.push({
				type: 'server',
				timestamp: Date.now(),
				message: `${nick} quit`
			});
		}
	}
}

type ThunkApiConfig = {
	state: {
		irc: ReturnType<typeof ircSlice.getInitialState>;
	};
	extra: { ircClient: IrcClient };
};

export const setUser = createAsyncThunk<IrcUserInfo, IrcUserInfo, ThunkApiConfig>(
	'irc/setUser',
	async (args, api) => {
		const { ircClient } = api.extra;
		try {
			ircClient.send(`USER ${args.username || 0} 0 0 :${args.realname}`);
		} catch (err: unknown) {
			return api.rejectWithValue(String(err));
		}
		return api.fulfillWithValue(args);
	}
);

export const setNick = createAsyncThunk<string, string, ThunkApiConfig>(
	'irc/setNick',
	async (nick, api) => {
		const { ircClient } = api.extra;
		try {
			ircClient.send(`NICK ${nick}`);
		} catch (err: unknown) {
			return api.rejectWithValue(String(err));
		}
		return api.fulfillWithValue(nick);
	}
);

export const processIncoming = createAsyncThunk<IrcEvent, string, ThunkApiConfig>(
	'irc/processIncoming',
	async (raw, api) => {
		const { ircClient } = api.extra;
		const message = parseIrcMessage(raw);
		const event = toIrcEvent(message);
		if (event.type === 'ping') {
			try {
				ircClient.send(`PONG :${event.id}`);
			} catch (err: unknown) {
				return api.rejectWithValue(String(err));
			}
		}
		return api.fulfillWithValue(event);
	}
);

export const messageChannel = createAsyncThunk<
	IrcLog,
	{ channel: string; message: string },
	ThunkApiConfig
>('irc/kickUser', async ({ channel, message }, thunkAPI) => {
	try {
		thunkAPI.extra.ircClient.send(`PRIVMSG ${channel} :${message}`);
		const state = thunkAPI.getState();
		const from = state.irc.connection.nick;
		if (from) {
			return thunkAPI.fulfillWithValue({
				timestamp: Date.now(),
				type: 'outgoing',
				to: channel,
				message,
				from
			});
		}
		return thunkAPI.rejectWithValue('Cannot send message without nick.');
	} catch (err: unknown) {
		return thunkAPI.rejectWithValue(String(err));
	}
});

function selectSelf(state: ReturnType<typeof ircSlice.getInitialState>) {
	return state;
}

export const selectSystemMessages = createSelector(
	[selectSelf],
	(rootState) => rootState.systemMessages
);

export const selectChannelMessages = createSelector(
	[selectSelf, (_, channel: string) => channel],
	(rootState, channel: string) => rootState.channels[channel]?.logs
);

export const getChannels = createSelector([selectSelf], (rootState) => rootState.channels);
export const getConnectionState = createSelector([selectSelf], (rootState) => rootState.connection);

export const getUnreadCount = createSelector([getChannels], (channels) =>
	Object.values(channels).reduce((prev, curr) => prev + curr.unread || 0, 0)
);

export const getChannelUsers = createSelector(
	[getChannels, (_, name: string) => name],
	(channels, name) => channels[name]?.users
);

export const { setConnectionState, markChannelOpen, clearNames } = ircSlice.actions;

export const ircReducer = ircSlice.reducer;
