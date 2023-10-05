import type { EventCallback, EventName } from '@tauri-apps/api/event';
import { invoke as invokeTauri } from '@tauri-apps/api/tauri';
import { listen as listenTauri } from '@tauri-apps/api/event';

export enum Code {
	Unknown = 'errors.unknown',
	PushFailed = 'errors.push',
	FetchFailed = 'errors.fetch',
	Conflicting = 'errors.conflict',
	GitAutenticationFailed = 'errors.git.authentication',
	InvalidHead = 'errors.git.head',
	Projects = 'errors.projects'
}

export class UserError extends Error {
	code!: Code;

	constructor(message: string, code: Code, cause: unknown) {
		super(message);

		this.name = 'UserError';
		this.cause = cause;
		this.code = code;
	}

	static fromError(error: any): UserError {
		const cause = error instanceof Error ? error : undefined;
		const code = error.code ?? Code.Unknown;
		const message = error.message ?? 'Unknown error';
		return new UserError(message, code, cause);
	}
}

export async function invoke<T>(command: string, params: Record<string, unknown> = {}): Promise<T> {
	// This commented out code can be used to delay/reject an api call
	// return new Promise<T>((resolve, reject) => {
	// 	if (command.startsWith('apply')) {
	// 		setTimeout(() => {
	// 			reject('rejected');
	// 		}, 2000);
	// 	} else {
	// 		resolve(invokeTauri<T>(command, params));
	// 	}
	// }).catch((reason) => {
	// 	const userError = UserError.fromError(reason);
	// 	console.error(`ipc->${command}: ${JSON.stringify(params)}`, userError);
	// 	throw userError;
	// });
	return (
		invokeTauri<T>(command, params)
			// .then((value) => {
			// 	console.debug(`ipc->${command}(${JSON.stringify(params)})`, value);
			// 	return value;
			// })
			.catch((reason) => {
				const userError = UserError.fromError(reason);
				console.error(`ipc->${command}: ${JSON.stringify(params)}`, userError);
				throw userError;
			})
	);
}

export function listen<T>(event: EventName, handle: EventCallback<T>) {
	const unlisten = listenTauri(event, handle);
	return () => unlisten.then((unlistenFn) => unlistenFn());
}
