import { Cmd, parseIRCMessage, toIrcEvent, type IrcEvent } from '$lib/irc/parser';
import { InjectionToken } from '@gitbutler/core/context';
import { reactive } from '@gitbutler/shared/reactiveUtils.svelte';
import ReconnectingWebSocket, { type CloseEvent, type ErrorEvent } from 'reconnecting-websocket';

export const connecting = ReconnectingWebSocket['CONNECTING'];
export const open = ReconnectingWebSocket['OPEN'];
export const closing = ReconnectingWebSocket['CLOSING'];
export const closed = ReconnectingWebSocket['CLOSED'];

export enum ReadyState {
	Connecting = connecting,
	Open = open,
	Closing = closing,
	Closed = closed
}

const capabilities = [
	'account-tag',
	'message-tags',
	'server-time',
	'standard-replies',
	'echo-message',
	'away-notify',
	'account-tag',
	'account-notify',
	'invite-notify',
	'labeled-response'
];

export const IRC_CLIENT = new InjectionToken<IrcClient>('IrcClient');

/**
 * A service for tracking uncommitted changes.
 *
 * Since we want to maintain a list and access individual records we use a
 * redux entity adapter on the results.
 */
export class IrcClient {
	private _connected = $state(false);
	private _error = $state<string>();
	private _server = $state<string>();

	private registered = false;

	private nick?: string;

	private socket: ReconnectingWebSocket | undefined;
	private onEventSubscribers: ((message: IrcEvent) => void)[] = [];
	private onOpenSubscribers: (() => void)[] = [];

	constructor() {}

	get connected() {
		return reactive(() => this._connected);
	}

	/**
	 * Connect to an IRC server.
	 *
	 * Throws if identity not available, the server otherwise disconnects
	 * after some timeout.
	 */
	connect(config: { server: string; nick: string }) {
		const socket = new ReconnectingWebSocket(config.server, [], {
			maxRetries: 0,
			minReconnectionDelay: 5000
		});
		socket.onopen = this.onopen_.bind(this);
		socket.onmessage = this.onmessage.bind(this);
		socket.onclose = this.onclose.bind(this);
		socket.onerror = this.onerror.bind(this);
		this._server = config.server;
		this.socket = socket;
		this.nick = config.nick;
	}

	private async onopen_() {
		this._server = this.socket?.url;
		this.send('CAP LS 302');
	}

	private onregistered() {
		this._connected = true;
		this.onOpenSubscribers.forEach((subscriber) => subscriber());
		console.warn('IRC connection open');
	}

	private handleUnathenticated() {}

	onopen(callback: () => void): () => void {
		this.onOpenSubscribers.push(callback);
		return () => {
			this.onOpenSubscribers.splice(this.onOpenSubscribers.indexOf(callback));
		};
	}

	private async onmessage(message: MessageEvent) {
		const data = message.data as string;
		const ircMessage = parseIRCMessage(data);
		const event = toIrcEvent(ircMessage);

		if (event.type === 'ping') {
			this.send(`${Cmd.PONG} :${event.id}`);
			return;
		}

		if (!this.registered) {
			if (event.type === 'welcome') {
				this.registered = true;
				this.onregistered();
			} else if (event.type === 'capabilities' && event.subcommand === 'LS') {
				this.send(`CAP REQ : ${capabilities.join(' ')}`);
			} else if (event.type === 'capabilities' && event.subcommand === 'NAK') {
				this._error = 'Capabilities rejected!';
				return;
			} else if (event.type === 'capabilities' && event.subcommand === 'ACK') {
				this.send('CAP END');
				this.send(`NICK ${this.nick}`);
				this.send(`USER ${this.nick} 0 * :${this.nick}`);
			}
			return;
		}

		this.onEventSubscribers.forEach((cb) => {
			cb(event);
		});
	}

	private onclose(_: CloseEvent) {
		this._connected = false;
		console.warn('IRC connection closed');
	}

	private onerror(event: ErrorEvent) {
		console.error(event);
	}

	onevent(callback: (message: IrcEvent) => void): () => void {
		this.onEventSubscribers.push(callback);
		return () => {
			this.onEventSubscribers.splice(this.onEventSubscribers.indexOf(callback));
		};
	}

	async send(message: string) {
		if (!this.socket) {
			throw new Error('Websocket not connected!');
		}
		this.socket.send(message);
	}

	disconnect() {
		this.socket?.close();
	}

	get status(): ReadyState {
		return (this.socket?.readyState as ReadyState) || ReadyState.Closed;
	}

	get server() {
		return reactive(() => this._server);
	}
}
