import { Tauri } from '$lib/backend/tauri';
import { isReduxError, type ReduxError } from '$lib/state/reduxError';
import { isErrorlike } from '@gitbutler/ui/utils/typeguards';
import { type BaseQueryApi, type QueryReturnValue } from '@reduxjs/toolkit/query';
import type { ExtraOptions } from '$lib/state/butlerModule';

import type { BaseQueryFn } from '@reduxjs/toolkit/query';

export type TauriExtraOptions = ExtraOptions & { command?: string };
export type TauriBaseQueryFn = BaseQueryFn<ApiArgs, unknown, unknown, TauriExtraOptions>;

// eslint-disable-next-line func-style
export const tauriBaseQuery: TauriBaseQueryFn = async (
	args: ApiArgs,
	api: BaseQueryApi,
	extra: TauriExtraOptions
): Promise<QueryReturnValue<unknown, ReduxError, undefined>> => {
	const command = extra.command;
	if (!command) {
		return newError('Expected a command!');
	}

	if (!hasTauriExtra(api.extra)) {
		return newError('Redux dependency Tauri not found!');
	}

	try {
		const result = { data: await api.extra.tauri.invoke(command, args) };
		return result;
	} catch (error: unknown) {
		const name = `API error: (${command})`;
		if (isReduxError(error)) {
			const newMessage =
				`command: ${command}\nparams: ${JSON.stringify(args)})\n\n` + error.message;
			return { error: { name, message: newMessage, code: error.code } };
		}

		if (isErrorlike(error)) {
			return { error: { name, message: error.message } };
		}

		return { error: { name, message: String(error) } };
	}
};

function newError(message: string) {
	return {
		error: { name: 'Failed to execute Tauri query', message }
	};
}

type ApiArgs = Record<string, unknown> | undefined;

/**
 * Typeguard for accessing injected Tauri dependency safely.
 */
export function hasTauriExtra(extra: unknown): extra is {
	tauri: Tauri;
} {
	return (
		!!extra &&
		typeof extra === 'object' &&
		extra !== null &&
		'tauri' in extra &&
		extra.tauri instanceof Tauri
	);
}
