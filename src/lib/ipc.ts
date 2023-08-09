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
	ProjectCreateFailed = 'errors.projects.create'
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
	// console.log('listen', event);
	return () => {
		unlisten.then((unlisten) => {
			unlisten();
			// console.log('unlisten', event);
		});
	};
}
