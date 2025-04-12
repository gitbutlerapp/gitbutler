import { PostHogWrapper } from '$lib/analytics/posthog';
import { Tauri } from '$lib/backend/tauri';
import { isBackendError } from '$lib/error/typeguards';
import { type BaseQueryApi, type QueryReturnValue } from '@reduxjs/toolkit/query';

export type TauriBaseQueryFn = typeof tauriBaseQuery;

export async function tauriBaseQuery(
	args: ApiArgs,
	api: BaseQueryApi
): Promise<QueryReturnValue<unknown, TauriCommandError, undefined>> {
	if (!hasTauriExtra(api.extra)) {
		return { error: { message: 'Redux dependency Tauri not found!' } };
	}

	const posthog = hasPosthogExtra(api.extra) ? api.extra.posthog : undefined;

	try {
		const result = { data: await api.extra.tauri.invoke(args.command, args.params) };
		if (posthog && args.actionName) {
			posthog.capture(`${args.actionName} Successful`);
		}
		return result;
	} catch (error: unknown) {
		if (isBackendError(error)) {
			const result = { error: { message: error.message, code: error.code } };
			if (posthog && args.actionName) {
				posthog.capture(`${args.actionName} Failed`, result);
			}
			return result;
		}

		const result = { error: { message: String(error) } };
		if (posthog && args.actionName) {
			posthog.capture(`${args.actionName} Failed`, result);
		}
		return result;
	}
}

type ApiArgs = {
	command: string;
	params: Record<string, unknown>;
	actionName?: string;
};

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

export function hasPosthogExtra(extra: unknown): extra is {
	posthog: PostHogWrapper;
} {
	return (
		!!extra &&
		typeof extra === 'object' &&
		extra !== null &&
		'posthog' in extra &&
		extra.posthog instanceof PostHogWrapper
	);
}
