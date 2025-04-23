import { GitHubClient } from '$lib/forge/github/githubClient';
import { DEFAULT_HEADERS } from '$lib/forge/github/headers';
import { isErrorlike, type ErrorLike } from '@gitbutler/ui/utils/typeguards';
import type { Octokit, RestEndpointMethodTypes } from '@octokit/rest';

type OwnerAndRepo = { owner: string; repo: string };

/**
 * Effectively the `BaseQueryFn` we want, but located in this service file
 * until we figure out a way of making the generic parameters work.
 *
 * TODO: Figure out a way of doing this without @ts-expect-error?
 */
export async function ghQuery<
	T extends keyof RestEndpointMethodTypes,
	S extends keyof RestEndpointMethodTypes[T],
	RequiresOwnerAndRepo extends 'required' | 'optional'
>(
	args:
		| GhArgs<T, S>
		| ((
				octokit: Octokit,
				about: RequiresOwnerAndRepo extends 'required' ? OwnerAndRepo : Partial<OwnerAndRepo>
		  ) => Promise<InferGhResponse<T, S>>),
	passedExtra?: unknown,
	requiresOwnerAndRepo?: RequiresOwnerAndRepo
): Promise<InferGhResponse<T, S>> {
	let extra;
	if (typeof args === 'function') {
		extra = passedExtra;
	} else {
		extra = args.extra;
	}

	if (!hasGitHub(extra)) throw new Error('No GitHub client!');
	const gh = extra.gitHubClient;
	try {
		let result;
		if (typeof args === 'function') {
			if ((!gh.owner || !gh.repo) && requiresOwnerAndRepo === 'required') {
				return { error: { message: 'No GitHub owner or repo!' } };
			}
			result = await args(gh.octokit, {
				owner: gh.owner,
				repo: gh.repo
			} as RequiresOwnerAndRepo extends 'required' ? OwnerAndRepo : Partial<OwnerAndRepo>);
		} else {
			// @ts-expect-error because of dynamic keys.
			result = await gh.octokit[args.domain][args.action]({
				...args.parameters,
				headers: DEFAULT_HEADERS,
				owner: gh.owner,
				repo: gh.repo
			});
		}

		return result;
	} catch (err: unknown) {
		return { error: isErrorlike(err) ? err.message : { message: String(err) } } as any;
	}
}

/**
 * Argument to `ghQuery` where parameters are inferred from octokit.js.
 */
export type GhArgs<
	T extends keyof RestEndpointMethodTypes,
	S extends keyof RestEndpointMethodTypes[T],
	O extends keyof RestEndpointMethodTypes[T][S] = keyof RestEndpointMethodTypes[T][S] &
		'parameters',
	P extends RestEndpointMethodTypes[T][S][O] = RestEndpointMethodTypes[T][S][O]
> = {
	domain: T;
	action: S;
	parameters?: Omit<P, 'owner' | 'string'>;
	extra: unknown;
};

export type GhResponse<T> = { data: T; error?: never } | { data?: never; error: ErrorLike };
/**
 * Response type for `ghQuery` inferred from octokit.js.
 */
export type InferGhResponse<
	T extends keyof RestEndpointMethodTypes,
	S extends keyof RestEndpointMethodTypes[T],
	O extends keyof RestEndpointMethodTypes[T][S] = keyof RestEndpointMethodTypes[T][S] & 'response',
	Q extends keyof RestEndpointMethodTypes[T][S][O] = keyof RestEndpointMethodTypes[T][S][O] &
		'data',
	P extends RestEndpointMethodTypes[T][S][O][Q] = RestEndpointMethodTypes[T][S][O][Q]
> = GhResponse<P>;

/**
 * Typeguard for accessing injected `GitHubClient` dependency safely.
 */
export function hasGitHub(extra: unknown): extra is {
	gitHubClient: GitHubClient;
} {
	return (
		!!extra &&
		typeof extra === 'object' &&
		extra !== null &&
		'gitHubClient' in extra &&
		extra.gitHubClient instanceof GitHubClient
	);
}
