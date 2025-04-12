export interface IRCUser {
	nick: string;
	user?: string;
	host?: string;
	mode?: 'op' | 'voice' | null; // Op (+@) or Voice (+)
}

export interface IrcChannel {
	name: string;
	users: Record<string, IRCUser>; // Keyed by nick
	logs: IrcLog[];
	topic?: string;
	// Number of new messages since last seen message.
	unread: number;
	// True if user is currently viewing this channel. It lets us skip
	// incrementing the unread counter.
	open?: boolean;
}

export type IrcLog = {
	type: string;
	timestamp: number;
} & (
	| {
			type: 'incoming';
			from: string;
			message: string;
			isCTCP?: boolean;
			ctcpCommand?: string;
			ctcpParams?: string[];
			ctcpType?: 'request' | 'reply';
	  }
	| { type: 'outgoing'; from: string; to: string; message: string; error?: any }
	| { type: 'server'; message: string }
	| { type: 'command'; raw?: string }
);

export interface IRCState {
	connection: {
		connected: boolean;
		nick?: string;
	};
	channels: Record<string, IrcChannel>;
	systemMessages: IrcLog[];
	whois: Record<string, any>; // Storing WHOIS info by nick
}

export type WhoInfo = { nick: string; username?: string; realname?: string };
export type IrcUserInfo = { username?: string; realname?: string };
