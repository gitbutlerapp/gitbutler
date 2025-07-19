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

export enum Code {
	Unknown = 'errors.unknown',
	Validation = 'errors.validation',
	ProjectsGitAuth = 'errors.projects.git.auth',
	DefaultTargetNotFound = 'errors.projects.default_target.not_found',
	CommitSigningFailed = 'errors.commit.signing_failed',
	ProjectMissing = 'errors.projects.missing'
}

export type TauriCommandError = { name: string; message: string; code?: string };

export function isTauriCommandError(something: unknown): something is TauriCommandError {
	return (
		!!something &&
		typeof something === 'object' &&
		something !== null &&
		'message' in something &&
		typeof (something as TauriCommandError).message === 'string' &&
		('code' in something ? typeof (something as TauriCommandError).code === 'string' : true)
	);
}

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
		if (import.meta.env.VITE_BUILD_TARGET === 'electron') {
			// TODO: Implement invoke
			const response = await fetch('http://localhost:6978', {
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
				throw new Error(String(out.subject));
			}
		} else {
			return await invokeTauri<T>(command, params);
		}
	} catch (error: unknown) {
		if (isTauriCommandError(error)) {
			console.error(`ipc->${command}: ${JSON.stringify(params)}`, error);
		}
		throw error;
	}
}

let webListener: WebListener | undefined;

export function listen<T>(event: EventName, handle: EventCallback<T>) {
	if (import.meta.env.VITE_BUILD_TARGET === 'electron') {
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

	listen(handler: { name: EventName; handle: EventCallback<any> }): () => void {
		this.handlers.push(handler);
		this.count++;
		if (!this.socket) {
			this.socket = new WebSocket('ws://localhost:6978/ws');
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

		return () => {
			this.handlers = this.handlers.filter((h) => h !== handler);
			this.count--;
			if (this.count === 0) {
				this.socket?.close();
				this.socket = undefined;
			}
		};
	}
}
