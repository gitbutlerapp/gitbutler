import { PostHogWrapper } from '$lib/analytics/posthog';
import { isTauriCommandError, type TauriCommandError } from '$lib/backend/ipc';
import { Tauri } from '$lib/backend/tauri';
import { isErrorlike } from '@gitbutler/ui/utils/typeguards';
import { type BaseQueryApi, type QueryReturnValue } from '@reduxjs/toolkit/query';

export type TauriBaseQueryFn = typeof tauriBaseQuery;

export async function tauriBaseQuery(
	args: ApiArgs,
	api: BaseQueryApi
): Promise<QueryReturnValue<unknown, TauriCommandError, undefined>> {
	if (!hasTauriExtra(api.extra)) {
		return {
			error: { name: 'Failed to execute Tauri query', message: 'Redux dependency Tauri not found!' }
		};
	}
	const posthog = hasPosthogExtra(api.extra) ? api.extra.posthog : undefined;

	const startTime = Date.now();
	try {
		const result = { data: await api.extra.tauri.invoke(args.command, args.params) };
		const durationMs = Date.now() - startTime;
		posthog?.capture('tauri_command', {
			command: args.command,
			durationMs,
			failure: false
		});
		if (posthog && args.actionName) {
			posthog.capture(`${args.actionName} Successful`, {
				durationMs
			});
		}
		return result;
	} catch (error: unknown) {
		const durationMs = Date.now() - startTime;
		posthog?.capture('tauri_command', {
			command: args.command,
			durationMs,
			failure: true
		});
		if (posthog && args.actionName) {
			posthog.capture(`${args.actionName} Failed`, { error });
		}

		const name = `API error: ${args.actionName} (${args.command})`;
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
