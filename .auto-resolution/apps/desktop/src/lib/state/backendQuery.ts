import { PostHogWrapper } from '$lib/analytics/posthog';
import { isTauriCommandError, type TauriCommandError } from '$lib/backend/ipc';
import { Tauri } from '$lib/backend/tauri';
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
		if (isTauriCommandError(error)) {
			if (posthog && args.actionName) {
				posthog.capture(`${args.actionName} Failed`, { error });
			}
			throw error;
		}
		const newError = { message: String(error) };
		if (posthog && args.actionName) {
			posthog.capture(`${args.actionName} Failed`, { error: newError });
		}
		throw newError;
	}
}

type ApiArgs = {
	command: string;
	params: Record<string, unknown>;
	actionName?: string;
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
