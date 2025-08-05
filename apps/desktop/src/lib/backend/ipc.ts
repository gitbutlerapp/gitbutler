import { Code } from '$lib/error/knownErrors';
import { isReduxError } from '$lib/state/reduxError';
import { getCookie } from '$lib/utils/cookies';
import { invoke as invokeTauri } from '@tauri-apps/api/core';
import { listen as listenTauri } from '@tauri-apps/api/event';
import type { EventCallback, EventName } from '@tauri-apps/api/event';

type ServerResonse<T> =
	| {
			type: 'success';
			subject: T;
	  }
	| {
			type: 'error';
			subject: unknown;
	  };

export class UserError extends Error {
	code!: Code;
	cause: Error | undefined;

	constructor(message: string, code: Code, cause: Error | undefined) {
		super(message);
		this.cause = cause;
		this.code = code;
	}

	static fromError(error: any): UserError {
		const cause = error instanceof Error ? error : undefined;
		const code = error.code ?? Code.Unknown;
		const message = error.message ?? error;
		return new UserError(message, code, cause);
	}
}

export function getUserErrorCode(error: unknown): Code | undefined {
	if (error instanceof UserError) {
		return error.code;
	}
	const userError = UserError.fromError(error);
	return userError.code;
}

export async function invoke<T>(command: string, params: Record<string, unknown> = {}): Promise<T> {
	// This commented out code can be used to delay/reject an api call
	// return new Promise<T>((resolve, reject) => {
	// 	if (command.startsWith('apply')) {
	// 		setTimeout(() => {
	// 			reject('testing the error page');
	// 		}, 500);
	// 	} else {
	// 		resolve(invokeTauri<T>(command, params));
	// 	}
	// }).catch((reason) => {
	// 	const userError = UserError.fromError(reason);
	// 	console.error(`ipc->${command}: ${JSON.stringify(params)}`, userError);
	// 	throw userError;
	// });

	try {
		if (import.meta.env.VITE_BUILD_TARGET === 'web') {
			const response = await fetch(`http://${getWebUrl()}`, {
				method: 'POST',
				headers: {
					'Content-Type': 'application/json'
				},
				body: JSON.stringify({ command, params })
			});
			const out: ServerResonse<T> = await response.json();
			if (out.type === 'success') {
				return out.subject;
			} else {
				if (isReduxError(out.subject)) {
					console.error(`ipc->${command}: ${JSON.stringify(params)}`, out.subject);
				}
				throw out.subject;
			}
		} else {
			return await invokeTauri<T>(command, params);
		}
	} catch (error: unknown) {
		if (isReduxError(error)) {
			console.error(`ipc->${command}: ${JSON.stringify(params)}`, error);
		}
		throw error;
	}
}

let webListener: WebListener | undefined;

export function listen<T>(event: EventName, handle: EventCallback<T>) {
	if (import.meta.env.VITE_BUILD_TARGET === 'web') {
		if (!webListener) {
			webListener = new WebListener();
		}

		// TODO: Listening in electron
		return webListener.listen({ name: event, handle });
	} else {
		const unlisten = listenTauri(event, handle);
		return async () => await unlisten.then((unlistenFn) => unlistenFn());
	}
}

class WebListener {
	private socket: WebSocket | undefined;
	private count = 0;
	private handlers: { name: EventName; handle: EventCallback<any> }[] = [];

	listen(handler: { name: EventName; handle: EventCallback<any> }): () => Promise<void> {
		this.handlers.push(handler);
		this.count++;
		if (!this.socket) {
			this.socket = new WebSocket(`ws://${getWebUrl()}/ws`);
			this.socket.addEventListener('message', (event) => {
				const data: { name: string; payload: any } = JSON.parse(event.data);
				for (const handler of this.handlers) {
					if (handler.name === data.name) {
						// The id is an artifact from tauri, we don't use it so
						// I've used a random value
						handler.handle({ event: data.name, payload: data.payload, id: 69 });
					}
				}
			});
		}

		// This needs to be async just so it's the same API as the tauri version
		return async () => {
			this.handlers = this.handlers.filter((h) => h !== handler);
			this.count--;
			if (this.count === 0) {
				this.socket?.close();
				this.socket = undefined;
			}
		};
	}
}

function getWebUrl(): string {
	const host = getCookie('butlerHost') || import.meta.env.VITE_BUTLER_HOST || 'localhost';
	const port = getCookie('butlerPort') || import.meta.env.VITE_BUTLER_PORT || '6978';
	return `${host}:${port}`;
}
