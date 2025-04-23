// Enum of IRC commands and numeric reply names.
export enum Cmd {
	// Connection registration
	PASS = 'PASS',
	NICK = 'NICK',
	USER = 'USER',
	OPER = 'OPER',
	MODE = 'MODE',
	SERVICE = 'SERVICE',
	QUIT = 'QUIT',
	SQUIT = 'SQUIT',

	// Channel operations
	JOIN = 'JOIN',
	PART = 'PART',
	TOPIC = 'TOPIC',
	NAMES = 'NAMES',
	LIST = 'LIST',
	INVITE = 'INVITE',
	KICK = 'KICK',

	// Messaging
	PRIVMSG = 'PRIVMSG',
	NOTICE = 'NOTICE',

	// Server queries and commands
	MOTD = 'MOTD',
	LUSERS = 'LUSERS',
	VERSION = 'VERSION',
	STATS = 'STATS',
	LINKS = 'LINKS',
	TIME = 'TIME',
	CONNECT = 'CONNECT',
	TRACE = 'TRACE',
	ADMIN = 'ADMIN',
	INFO = 'INFO',

	// Service-specific
	SERVLIST = 'SERVLIST',
	SQUERY = 'SQUERY',

	// User-based queries
	WHO = 'WHO',
	WHOIS = 'WHOIS',
	WHOWAS = 'WHOWAS',

	// Miscellaneous
	KILL = 'KILL',
	PING = 'PING',
	PONG = 'PONG',
	ERROR = 'ERROR',

	// Capability negotiation (extensions)
	CAP = 'CAP',

	// Authentication (extensions)
	AUTHENTICATE = 'AUTHENTICATE',
	ACCOUNT = 'ACCOUNT',

	// Client-to-client protocol (CTCP) - sent via PRIVMSG/NOTICE
	CTCP = 'CTCP', // not an actual command, but useful to track it

	// Other common extensions
	AWAY = 'AWAY',
	REHASH = 'REHASH',
	DIE = 'DIE',
	RESTART = 'RESTART',
	WALLOPS = 'WALLOPS',
	USERHOST = 'USERHOST',
	ISON = 'ISON',

	// SASL authentication (used with CAP)
	RPL_LOGGEDIN = '900',
	RPL_LOGGEDOUT = '901',
	ERR_NICKLOCKED = '902',
	RPL_SASLSUCCESS = '903',
	ERR_SASLFAIL = '904',
	ERR_SASLTOOLONG = '905',
	ERR_SASLABORTED = '906',
	ERR_SASLALREADY = '907',

	// BATCH (used for grouping messages)
	BATCH = 'BATCH',

	// Extended JOIN (with account names)
	EJOIN = 'EJOIN',

	// Monitor (modern alternative to ISON)
	MONITOR = 'MONITOR',

	// --- 001–005: Connection and Welcome ---
	RPL_WELCOME = '001', // Welcome to the network
	RPL_YOURHOST = '002', // Host info
	RPL_CREATED = '003', // Server creation time
	RPL_MYINFO = '004', // Server info and supported modes
	RPL_ISUPPORT = '005', // Server-supported features

	// --- "311"–319: WHOIS/WHOWAS Replies ---
	RPL_WHOISUSER = '311', // WHOIS user info
	RPL_WHOISSERVER = '312', // WHOIS server
	RPL_WHOISOPERATOR = '313', // WHOIS is IRC operator
	RPL_WHOISIDLE = '317', // WHOIS idle + signon time
	RPL_ENDOFWHOIS = '318', // End of WHOIS
	RPL_WHOISCHANNELS = '319', // WHOIS channel list

	RPL_WHOWASUSER = '314', // WHOWAS info
	RPL_ENDOFWHOWAS = '369', // End of WHOWAS

	// --- "321"–323: LIST Replies ---
	RPL_LISTSTART = '321', // Start of LIST
	RPL_LIST = '322', // LIST entry
	RPL_LISTEND = '323', // End of LIST

	// --- "324"–329: MODE Replies ---
	RPL_CHANNELMODEIS = '324', // Current channel modes
	RPL_CREATIONTIME = '329', // Channel creation time

	// --- "331"–333: TOPIC Replies ---
	RPL_NOTOPIC = '331', // No topic is set
	RPL_TOPIC = '332', // Channel topic
	RPL_TOPICWHOTIME = '333', // Who set the topic and when

	// --- "341": INVITE Replies ---
	RPL_INVITING = '341', // INVITE confirmation

	// --- "346"–347: INVITELIST Replies ---
	RPL_INVITELIST = '346', // Invitation list entry
	RPL_ENDOFINVITELIST = '347', // End of invitation list

	// --- "352"–315: WHO Replies ---
	RPL_WHOREPLY = '352', // WHO reply line
	RPL_ENDOFWHO = '315', // End of WHO

	// --- "353"–366: NAMES Replies ---
	RPL_NAMREPLY = '353', // Channel name list
	RPL_ENDOFNAMES = '366', // End of NAMES list

	// --- "364"–365: LINKS Replies ---
	RPL_LINKS = '364', // LINK info
	RPL_ENDOFLINKS = '365', // End of LINKS

	// --- "367"–368: Ban List ---
	RPL_BANLIST = '367', // Ban list entry
	RPL_ENDOFBANLIST = '368', // End of ban list

	// --- "371"–376: INFO / MOTD ---
	RPL_INFO = '371', // Info text
	RPL_MOTDSTART = '375', // Start of MOTD
	RPL_MOTD = '372', // MOTD text line
	RPL_ENDOFINFO = '374', // End of INFO
	RPL_ENDOFMOTD = '376', // End of MOTD

	// --- "381": OPER ---
	RPL_YOUREOPER = '381', // You've successfully OPERed

	// --- "391": TIME ---
	RPL_TIME = '391', // Server time

	// --- "401"–406: Basic Errors ---
	ERR_NOSUCHNICK = '401', // No such nick/channel
	ERR_NOSUCHSERVER = '402', // No such server
	ERR_NOSUCHCHANNEL = '403', // No such channel
	ERR_CANNOTSENDTOCHAN = '404', // Cannot send to channel
	ERR_TOOMANYCHANNELS = '405', // Too many channels joined
	ERR_WASNOSUCHNICK = '406', // No such nickname

	// --- "411"–416: Message Errors ---
	ERR_NORECIPIENT = '411', // No recipient given
	ERR_NOTEXTTOSEND = '412', // No message text
	ERR_UNKNOWNCOMMAND = '421', // Unknown command

	// --- "432"–433: Nickname Errors ---
	ERR_ERRONEUSNICKNAME = '432', // Invalid nickname
	ERR_NICKNAMEINUSE = '433', // Nickname already in use

	// --- "441"–443: Channel User Errors ---
	ERR_USERNOTINCHANNEL = '441', // User not in channel
	ERR_NOTONCHANNEL = '442', // Not on channel
	ERR_USERONCHANNEL = '443', // User already on channel

	// --- "451": Registration Errors ---
	ERR_NOTREGISTERED = '451', // Not registered

	// --- "461"–462: Parameter/Registration Errors ---
	ERR_NEEDMOREPARAMS = '461', // Not enough parameters
	ERR_ALREADYREGISTRED = '462', // Already registered

	// --- "464": Auth Errors ---
	ERR_PASSWDMISMATCH = '464', // Password incorrect

	// --- "471"–475: Channel Join Errors ---
	ERR_CHANNELISFULL = '471', // Channel is full
	ERR_INVITEONLYCHAN = '473', // Invite-only channel
	ERR_BANNEDFROMCHAN = '474', // Banned from channel
	ERR_BADCHANNELKEY = '475', // Incorrect channel key

	// --- "481"–482: Privilege Errors ---
	ERR_NOPRIVILEGES = '481', // No privileges
	ERR_CHANOPRIVSNEEDED = '482', // Channel operator privileges needed

	// --- "501"–502: Mode Errors ---
	ERR_UMODEUNKNOWNFLAG = '501', // Unknown user mode
	ERR_USERSDONTMATCH = '502' // Cannot change mode for others
}

export type IrcMessage = {
	raw: string;
	prefix?: string;
	command: string;
	params: string[];
	trailing?: string;
	numeric?: number; // If it's a numeric reply, this is set
	tags: Record<string, string>;
};

type IrcUser = {
	nick: string;
	user: string;
	host: string;
};

function prefixToUser(prefix: string | undefined): IrcUser | undefined {
	if (!prefix) return;
	const [nick, userHost] = prefix.split('!');
	const [userName, host] = userHost?.split('@') ?? [];
	return { nick: nick!, user: userName!, host: host! };
}

export type IrcEvent =
	| { type: 'userJoined'; nick: string; channel: string; user: IrcUser }
	| { type: 'userParted'; nick: string; channel: string; reason?: string }
	| { type: 'userQuit'; nick: string; reason?: string }
	| {
			type: 'groupMessage';
			from: string;
			to: string;
			text: string;
			msgid?: string;
			data?: unknown;
	  }
	| {
			type: 'privateMessage';
			from: string;
			to: string;
			text: string;
			msgid?: string;
			data?: unknown;
	  }
	| { type: 'namesList'; channel: string; names: string[] }
	| { type: 'capabilities'; capabilities: string[]; subcommand?: string }
	| { type: 'nickChanged'; oldNick: string; newNick: string }
	| { type: 'channelTopic'; channel: string; topic: string }
	| { type: 'whois'; nick: string; username?: string; realname?: string; host?: string }
	| { type: 'welcome'; nick: string; message: string }
	| { type: 'serverNotice'; target: string; message: string }
	| { type: 'motd'; message: string }
	| { type: 'error'; message: string; code: number; nick?: string; params?: string }
	| { type: 'ping'; id: string }
	| { type: 'unsupported'; command: string; raw: string }; // fallback/default

export function toIrcEvent(msg: IrcMessage): IrcEvent {
	const [nick] = msg.prefix?.split('!') ?? [];
	const user = prefixToUser(msg.prefix);

	switch (msg.command) {
		case Cmd.PING:
			return {
				type: 'ping',
				id: msg.trailing!
			};
		case Cmd.JOIN:
			return {
				type: 'userJoined',
				nick: nick ?? 'unknown',
				channel: msg.trailing!,
				user: user!
			};

		case Cmd.PART:
			return {
				type: 'userParted',
				nick: nick ?? 'unknown',
				channel: msg.trailing!,
				reason: msg.params[0]
			};

		case Cmd.QUIT:
			return {
				type: 'userQuit',
				nick: nick ?? 'unknown',
				reason: msg.trailing
			};

		case Cmd.PRIVMSG: {
			const target = msg.params[0]!;
			if (target.startsWith('#')) {
				return {
					type: 'groupMessage',
					from: nick ?? 'unknown',
					to: msg.params[0]!,
					text: msg.trailing || '',
					msgid: msg.tags['msgid'],
					data: msg.tags['+data']
				};
			}
			return {
				type: 'privateMessage',
				from: nick ?? 'unknown',
				to: msg.params[0]!,
				text: msg.trailing || '',
				msgid: msg.tags['msgid'],
				data: msg.tags['+data']
			};
		}
		case Cmd.CAP:
			return {
				type: 'capabilities',
				subcommand: msg.params.at(1),
				capabilities: msg.trailing?.split(' ') || []
			};

		case Cmd.NICK:
			return {
				type: 'nickChanged',
				oldNick: nick ?? 'unknown',
				newNick: msg.trailing || msg.params[0]!
			};

		case Cmd.TOPIC:
			return {
				type: 'channelTopic',
				channel: msg.params[0]!,
				topic: msg.trailing || ''
			};

		case Cmd.RPL_WELCOME:
			return {
				type: 'welcome',
				message: msg.trailing || '',
				nick: msg.params[0]!
			};

		case Cmd.NOTICE:
			return {
				type: 'serverNotice',
				target: msg.params[0]!,
				message: msg.trailing || ''
			};

		case Cmd.RPL_WHOISUSER: {
			const [_, nick, username, host, , realname] = msg.params;
			return {
				type: 'whois',
				nick: nick!,
				username,
				host,
				realname
			};
		}

		case Cmd.RPL_NAMREPLY: {
			return {
				type: 'namesList',
				channel: msg.params[2]!,
				names: msg.trailing!.split(' ')
			};
		}

		case Cmd.ERROR: {
			return {
				type: 'error',
				code: 0,
				message: msg.trailing || ''
			};
		}

		case Cmd.RPL_MOTDSTART:
		case Cmd.RPL_MOTD:
		case Cmd.RPL_ENDOFMOTD:
			return {
				type: 'motd',
				message: msg.trailing!
			};

		default:
			if (/^[45]\d{2}$/.test(msg.command)) {
				return {
					type: 'error',
					code: parseInt(msg.command, 10),
					nick: msg.params[0]!,
					params: msg.params.slice(1).join(' '),
					message: msg.trailing || ''
				};
			}

			return {
				type: 'unsupported',
				command: msg.command,
				raw: msg.raw
			};
	}
}

export function parseIRCMessage(line: string): IrcMessage {
	const raw = line;
	let rest = line.trim();
	let tags: Record<string, string | true> | undefined;
	let prefix: string | undefined;
	let command = '';
	const params: string[] = [];
	let trailing: string | undefined;

	// Extract tags.
	if (rest.startsWith('@')) {
		const spaceIdx = rest.indexOf(' ');
		const tagStr = rest.slice(1, spaceIdx);
		rest = rest.slice(spaceIdx + 1).trim();

		tags = {};
		for (const tag of tagStr.split(';')) {
			const [key, value] = tag.split('=');
			tags[key!] = value !== undefined ? value : true;
		}
	}

	// Extract prefix.
	if (rest.startsWith(':')) {
		const spaceIdx = rest.indexOf(' ');
		prefix = rest.slice(1, spaceIdx);
		rest = rest.slice(spaceIdx + 1).trim();
	}

	// Extract command.
	const parts = rest.split(' ');
	command = parts.shift()!;

	// Extract trailing.
	const trailingIndex = parts.findIndex((p) => p.startsWith(':'));
	if (trailingIndex !== -1) {
		params.push(...parts.slice(0, trailingIndex));
		trailing = parts.slice(trailingIndex).join(' ').slice(1);
	} else {
		params.push(...parts);
	}

	return {
		command: command as IrcMessage['command'],
		tags,
		prefix,
		params,
		trailing,
		raw
	} as IrcMessage;
}
