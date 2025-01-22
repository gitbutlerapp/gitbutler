import { Tauri } from '$lib/backend/tauri';
import { isBackendError } from '$lib/error/typeguards';
import { createApi, type BaseQueryApi, type BaseQueryFn } from '@reduxjs/toolkit/query';

export const reduxApi = createApi({
	reducerPath: 'api',
	tagTypes: ['Stacks'],
	baseQuery: tauriBaseQuery,
	endpoints: (_) => {
		return {};
	}
});

function tauriBaseQuery<T>(
	args: ApiArgs,
	api: BaseQueryApi
): ReturnType<BaseQueryFn<ApiArgs, Promise<T>, TauriCommandError, object, object>> {
	if (!hasTauriExtra(api.extra)) {
		return { error: { message: 'Redux dependency Tauri not found!' } };
	}
	try {
		return { data: api.extra.tauri.invoke(args.command, args.params) };
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
 *  Typeguard that makes `tauriBaseQuery` more concise.
 */
function hasTauriExtra(extra: unknown): extra is {
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
