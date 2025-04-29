import { logsAdapter } from '$lib/irc/logs';
import type { IrcChannel, IrcChat } from '$lib/irc/types';

export function createChannel(channels: Record<string, IrcChannel>, name: string): IrcChannel {
	const channel = {
		name,
		users: {},
		unread: 0,
		logs: logsAdapter.getInitialState()
	};
	channels[name] = channel;
	return channel;
}

export function createChat(chats: Record<string, IrcChat>, username: string): IrcChat {
	const chat = {
		username,
		unread: 0,
		logs: logsAdapter.getInitialState()
	};
	chats[username] = chat;
	return chat;
}

export function joinChannel(
	channels: Record<string, IrcChannel>,
	name: string,
	nick: string
): IrcChannel {
	let channel = channels[name];
	if (!channel) {
		channel = createChannel(channels, name);
	}
	channel.users[nick] = { nick };
	return channel;
}
