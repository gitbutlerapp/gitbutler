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

/**
 * A service for tracking uncommitted changes.
 *
 * Since we want to maintain a list and access individual records we use a
 * redux entity adapter on the results.
 */
export class IrcClient {
	private _connected = $state(false);
	get connected(): boolean {
		return this._connected;
	}

	private socket: ReconnectingWebSocket | undefined;
	private onMessageSubscribers: ((message: string) => void)[] = [];
	private onOpenSubscribers: (() => void)[] = [];

	constructor() {}

	private onopen_() {
		this._connected = true;
		this.onOpenSubscribers.forEach((subscriber) => subscriber());
		console.warn('IRC connection open');
	}

	onopen(callback: () => void): () => void {
		this.onOpenSubscribers.push(callback);
		return () => {
			this.onOpenSubscribers.splice(this.onOpenSubscribers.indexOf(callback));
		};
	}

	private async onmessage(event: MessageEvent) {
		const message = event.data as string;
		this.onMessageSubscribers.forEach((cb) => {
			cb(message);
		});
	}

	private onclose(_: CloseEvent) {
		this._connected = false;
		console.warn('IRC connection closed');
	}

	private onerror(event: ErrorEvent) {
		console.error(event);
	}

	onevent(callback: (message: string) => void): () => void {
		this.onMessageSubscribers.push(callback);
		return () => {
			this.onMessageSubscribers.splice(this.onMessageSubscribers.indexOf(callback));
		};
	}

	send(message: string) {
		if (!this.socket) {
			throw new Error('Websocket not connected!');
		}
		this.socket.send(message);
	}

	connect(server: string) {
		const socket = new ReconnectingWebSocket(server, [], {
			maxRetries: 20,
			minReconnectionDelay: 5000,
			startClosed: true
		});
		socket.onopen = this.onopen_.bind(this);
		socket.onmessage = this.onmessage.bind(this);
		socket.onclose = this.onclose.bind(this);
		socket.onerror = this.onerror.bind(this);
		this.socket = socket;
		this.socket.reconnect();
	}

	disconnect() {
		this.socket?.close();
	}

	get status(): ReadyState {
		return (this.socket?.readyState as ReadyState) || ReadyState.Closed;
	}

	get server(): string | undefined {
		return this.socket?.url;
	}
}
