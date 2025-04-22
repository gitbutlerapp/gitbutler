import { createChat, joinChannel } from '$lib/irc/channel';
import { logsAdapter, logSelectors } from '$lib/irc/logs';
import { type IrcEvent } from '$lib/irc/parser';
import {
	createAsyncThunk,
	createSelector,
	createSlice,
	type PayloadAction
} from '@reduxjs/toolkit';
import type { IrcClient } from '$lib/irc/ircClient.svelte';
import type { IRCState, IRCUser, IrcUserInfo } from '$lib/irc/types';

const initialState: IRCState = {
	connection: { connected: false },
	channels: {},
	chats: {},
	systemMessages: [],
	whois: {}
};

/**
 * This is basically a state machine that controls IRC messages.
 */
export const ircSlice = createSlice({
	name: 'irc',
	initialState,
	reducers: {
		setConnectionState(state, action: PayloadAction<boolean>) {
			state.connection.connected = action.payload;
		},
		markOpen(state, action: PayloadAction<{ name: string; open: boolean }>) {
			const name = action.payload.name;
			const target = name.startsWith('#') ? state.channels[name] : state.chats[name];
			if (target) {
				target.open = action.payload.open;
				if (target.open) {
					target.unread = 0;
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
		},
		processIncoming(state, action: PayloadAction<IrcEvent>) {
			const event = action.payload;
			const me = state.connection.nick!;

			switch (event.type) {
				case 'welcome': {
					state.connection = {
						nick: event.message,
						connected: true
					};
					break;
				}
				case 'userJoined': {
					const nick = event.user.nick;
					const channel = state.channels[event.channel];
					// When this client joins a new channale.
					if (!channel) joinChannel(state.channels, event.channel, nick);
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
				case 'groupMessage': {
					const channel = state.channels[event.to];
					if (!channel) {
						joinChannel(state.channels, event.to, event.to);
					}
					if (channel) {
						if (!channel.open) {
							channel.unread += 1;
						}
						logsAdapter.upsertOne(channel.logs, {
							type: event.to === me ? 'incoming' : 'outgoing',
							timestamp: Date.now(),
							msgid: event.msgid,
							from: event.from,
							to: event.to,
							data: event.data,
							message: event.text
						});
						// Trim server output to last 100 messages.
						while (channel.logs.ids.length > 100) {
							logsAdapter.removeMany(channel.logs, channel.logs.ids.slice(-100));
						}
					}
					break;
				}
				case 'privateMessage': {
					const { from, to } = event;
					const user = from === me ? to : from;
					let chat = state.chats[user];
					if (!chat) {
						chat = createChat(state.chats, from);
					}
					if (!chat.open) {
						chat.unread += 1;
					}
					chat.logs = logsAdapter.upsertOne(chat.logs, {
						type: event.to === me ? 'incoming' : 'outgoing',
						timestamp: Date.now(),
						msgid: event.msgid,
						from: event.from,
						to: event.to,
						data: event.data,
						message: event.text
					});
					// Trim server output to last 100 messages.
					if (chat.logs.ids.length > 100) {
						logsAdapter.removeMany(chat.logs, chat.logs.ids.slice(-100));
					}
					break;
				}
				case 'serverNotice': {
					const name = event.target;
					const channel = state.channels[name];
					if (channel) {
						channel.logs = logsAdapter.addOne(channel.logs, {
							type: 'notice',
							timestamp: Date.now(),
							message: event.message
						});
						// Trim server output to last 100 messages.
						while (channel.logs.ids.length > 100) {
							logsAdapter.removeMany(channel.logs, channel.logs.ids.slice(-100));
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
				case 'error': {
					state.systemMessages.push({
						timestamp: Date.now(),
						type: 'server',
						message: event.message
					});
					break;
				}

				case 'unsupported': {
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
			}

			// Trim server output to last 100 messages.
			while (state.systemMessages.length > 100) {
				state.systemMessages.shift();
			}
		}
	},
	extraReducers: (build) => {
		// Nick change.
		build.addCase(setNick.fulfilled, (state, action) => {
			state.connection.nick = action.payload;
		});
		// Sending a message to a channel failed.
		build.addCase(messageChannel.rejected, (state, action) => {
			const { channel: name, message } = action.meta.arg;
			let channel = state.channels[name];
			if (!channel) {
				channel = { name, logs: logsAdapter.getInitialState(), users: {}, unread: 0 };
				state.channels[name] = channel;
			}
			if (!channel.logs) {
				channel.logs = logsAdapter.getInitialState();
			}

			let errorMessage = action.error.message;
			if (action.payload) {
				errorMessage += '\n\n' + action.payload;
			}

			logsAdapter.addOne(channel.logs, {
				timestamp: Date.now(),
				type: 'outgoing',
				error: errorMessage,
				to: name,
				from: '', // TODO: What do we do here?
				message
			});
		});
		// Private message failed.
		build.addCase(messageNick.rejected, () => {
			// TODO: Show failed message.
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
			channel.logs = logsAdapter.addOne(channel.logs, {
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

export const messageChannel = createAsyncThunk<
	void,
	{ channel: string; message: string; data: unknown },
	ThunkApiConfig
>('irc/messageGroup', async ({ channel, message, data }, thunkAPI) => {
	let command = `PRIVMSG ${channel} :${message}`;
	try {
		if (data) {
			command = `@+data=${data} ${command}`;
		}
		thunkAPI.extra.ircClient.send(command);
		return thunkAPI.fulfillWithValue(undefined);
	} catch (err: unknown) {
		return thunkAPI.rejectWithValue(String(err));
	}
});

export const messageNick = createAsyncThunk<
	void,
	{ nick: string; message: string; data?: unknown },
	ThunkApiConfig
>('irc/messageNick', async ({ nick, message, data }, thunkAPI) => {
	let command = `PRIVMSG ${nick} :${message}`;
	try {
		if (data) {
			command = `@+data=${data} ${command}`;
		}
		thunkAPI.extra.ircClient.send(command);
		return thunkAPI.fulfillWithValue(undefined);
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
	(rootState, channel: string) => {
		const logs = rootState.channels[channel]?.logs;
		return logs ? logSelectors.selectAll(logs) : [];
	}
);

export const selectPrivateMessages = createSelector(
	[selectSelf, (_, channel: string) => channel],
	(rootState, nick: string) => {
		const logs = rootState.chats[nick]?.logs;
		return logs ? logSelectors.selectAll(logs) : [];
	}
);

export const getChats = createSelector([selectSelf], (rootState) => rootState.chats);
export const getChannels = createSelector([selectSelf], (rootState) => rootState.channels);
export const getConnectionState = createSelector([selectSelf], (rootState) => rootState.connection);

export const getUnreadCount = createSelector([getChannels], (channels) =>
	Object.values(channels).reduce((prev, curr) => prev + curr.unread || 0, 0)
);

export const getChannelUsers = createSelector(
	[getChannels, (_, name: string) => name],
	(channels, name) => channels[name]?.users
);

export const { setConnectionState, markOpen, clearNames, processIncoming } = ircSlice.actions;

export const ircReducer = ircSlice.reducer;
