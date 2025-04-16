import { PostHogWrapper } from '$lib/analytics/posthog';
import { isTauriCommandError, type TauriCommandError } from '$lib/backend/ipc';
import { Tauri } from '$lib/backend/tauri';
import { createTitledError, parseError } from '$lib/error/parser';
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
		const title = `API error: ${args.command}`;
		if (isTauriCommandError(error)) {
			if (posthog && args.actionName) {
				const title = `${args.actionName} Failed`;
				posthog.capture(title, { error });
				throw createTitledError(title, error);
			}
		}

		const newError = parseError(error);
		if (posthog && args.actionName) {
			const title = `${args.actionName} Failed`;
			posthog.capture(title, { error: newError });
			throw createTitledError(title, newError);
		}

		throw createTitledError(title, newError);
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
