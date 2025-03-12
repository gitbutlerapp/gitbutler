import { isBackendError } from '$lib/error/typeguards';
import { Octokit } from '@octokit/rest';
import { type BaseQueryApi, type QueryReturnValue } from '@reduxjs/toolkit/query';
import type { RestEndpointMethodTypes } from '@octokit/rest';

type ActionKey = keyof RestEndpointMethodTypes;
type MethodKey<T extends ActionKey> = keyof RestEndpointMethodTypes[T];

export async function githubBaseQuery<T extends ActionKey>(
	args: {
		action: T;
		method: RestEndpointMethodTypes[T];
		params: Record<string, unknown>;
	},
	api: BaseQueryApi
): Promise<QueryReturnValue<unknown, TauriCommandError, undefined>> {
	if (!hasOctokitExtra(api.extra)) {
		return { error: { message: 'Redux dependency Octokit not found!' } };
	}
	try {
		return { data: undefined };
	} catch (error: unknown) {
		if (isBackendError(error)) {
			return { error };
		}
		return { error: { message: String(error) } };
	}
}

type TauriCommandError = { message: string; code?: string };

/**
 * Typeguard for accessing injected Tauri dependency safely.
 */
export function hasOctokitExtra(extra: unknown): extra is {
	octokit: Octokit;
} {
	return (
		!!extra &&
		typeof extra === 'object' &&
		extra !== null &&
		'octokit' in extra &&
		extra.octokit instanceof Octokit
	);
}
