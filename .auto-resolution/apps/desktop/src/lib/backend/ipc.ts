import { invoke as invokeTauri } from '@tauri-apps/api/core';
import { listen as listenTauri } from '@tauri-apps/api/event';
import type { EventCallback, EventName } from '@tauri-apps/api/event';

export enum Code {
	Unknown = 'errors.unknown',
	Validation = 'errors.validation',
	ProjectsGitAuth = 'errors.projects.git.auth',
	DefaultTargetNotFound = 'errors.projects.default_target.not_found',
	CommitSigningFailed = 'errors.commit.signing_failed',
	ProjectMissing = 'errors.projects.missing'
}

export type TauriCommandError = { message: string; code?: string };

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

export function isUserErrorCode(something: unknown): something is Code {
	return Object.values(Code).includes(something as Code);
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
	return undefined;
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
		return await invokeTauri<T>(command, params);
	} catch (error: unknown) {
		if (isTauriCommandError(error)) {
			console.error(`ipc->${command}: ${JSON.stringify(params)}`, error);
		}
		throw error;
	}
}

export function listen<T>(event: EventName, handle: EventCallback<T>) {
	const unlisten = listenTauri(event, handle);
	return async () => await unlisten.then((unlistenFn) => unlistenFn());
}

export class CommandService {
	async invoke<T>(command: string, params: Record<string, unknown> = {}): Promise<T> {
		return await invoke<T>(command, params);
	}

	listen<T>(event: EventName, handle: EventCallback<T>): () => Promise<void> {
		return listen<T>(event, handle);
	}
}
