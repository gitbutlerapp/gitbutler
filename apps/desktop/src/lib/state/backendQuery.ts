import { Tauri } from '$lib/backend/tauri';
import { isBackendError } from '$lib/error/typeguards';
import { type BaseQueryApi, type QueryReturnValue } from '@reduxjs/toolkit/query';

export async function tauriBaseQuery(
	args: ApiArgs,
	api: BaseQueryApi
): Promise<QueryReturnValue<unknown, TauriCommandError, undefined>> {
	if (!hasTauriExtra(api.extra)) {
		return { error: { message: 'Redux dependency Tauri not found!' } };
	}
	try {
		return { data: await api.extra.tauri.invoke(args.command, args.params) };
	} catch (error: unknown) {
		if (isBackendError(error)) {
			return { error: { message: error.message, code: error.code } };
		}
		return { error: { message: String(error) } };
	}
}

type ApiArgs = {
	command: string;
	params: Record<string, unknown>;
};

type TauriCommandError = { message: string; code?: string };

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
