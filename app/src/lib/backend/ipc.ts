import { listen as listenTauri } from '@tauri-apps/api/event';
import { invoke as invokeTauri } from '@tauri-apps/api/tauri';
import { writable } from 'svelte/store';
import type { EventCallback, EventName } from '@tauri-apps/api/event';

export enum Code {
	Unknown = 'errors.unknown',
	Validation = 'errors.validation',
	Projects = 'errors.projects',
	ProjectsGitAuth = 'errors.projects.git.auth',
	ProjectsGitRemote = 'errors.projects.git.remote',
	ProjectHead = 'errors.projects.head',
	ProjectConflict = 'errors.projects.conflict'
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

interface LoadItem {
	name: string;
	startedAt: Date;
}
const loadingStore = writable(false);
export const loadStack: LoadItem[] = [];
export const isLoading = {
	...loadingStore,
	loadStack,
	push: (item: LoadItem) => {
		loadStack.push(item);
		loadingStore.set(true);
	},
	pop: (item: LoadItem) => {
		const i = loadStack.indexOf(item);
		loadStack.splice(i, 1);
		if (loadStack.length == 0) loadingStore.set(false);
	}
};

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
	const loadingItem = { name: command, startedAt: new Date() };
	isLoading.push(loadingItem);

	try {
		return await invokeTauri<T>(command, params);
	} catch (reason) {
		const userError = UserError.fromError(reason);
		console.error(`ipc->${command}: ${JSON.stringify(params)}`, userError, reason);
		throw userError;
	} finally {
		isLoading.pop(loadingItem);
	}
}

export function listen<T>(event: EventName, handle: EventCallback<T>) {
	const unlisten = listenTauri(event, handle);
	return async () => await unlisten.then((unlistenFn) => unlistenFn());
}
