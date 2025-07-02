import { isTauriCommandError, type TauriCommandError } from '$lib/backend/ipc';
import { Tauri } from '$lib/backend/tauri';
import { isErrorlike } from '@gitbutler/ui/utils/typeguards';
import { type BaseQueryApi, type QueryReturnValue } from '@reduxjs/toolkit/query';

import type { BaseQueryFn } from '@reduxjs/toolkit/query';

export type ExtraOptions = { [key: string]: string };
export type TauriBaseQueryFn = BaseQueryFn<ApiArgs, unknown, unknown, ExtraOptions | undefined>;

// eslint-disable-next-line func-style
export const tauriBaseQuery: TauriBaseQueryFn = async (
	args: ApiArgs,
	api: BaseQueryApi
): Promise<QueryReturnValue<unknown, TauriCommandError, undefined>> => {
	if (!hasTauriExtra(api.extra)) {
		return {
			error: { name: 'Failed to execute Tauri query', message: 'Redux dependency Tauri not found!' }
		};
	}

	try {
		const result = { data: await api.extra.tauri.invoke(args.command, args.params) };
		return result;
	} catch (error: unknown) {
		const name = `API error: (${args.command})`;
		if (isTauriCommandError(error)) {
			const newMessage =
				`command: ${args.command}\nparams: ${JSON.stringify(args.params)})\n\n` + error.message;
			return { error: { name, message: newMessage, code: error.code } };
		}

		if (isErrorlike(error)) {
			return { error: { name, message: error.message } };
		}

		return { error: { name, message: String(error) } };
	}
};

type ApiArgs = {
	command: string;
	params: Record<string, unknown>;
};

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
