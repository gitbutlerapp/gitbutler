import { GitHubClient } from '$lib/forge/github/githubClient';
import { DEFAULT_HEADERS } from '$lib/forge/github/headers';
import { isErrorlike, type ErrorLike } from '@gitbutler/ui/utils/typeguards';
import type { RestEndpointMethodTypes } from '@octokit/rest';

type Transformer<
	T extends keyof RestEndpointMethodTypes,
	S extends keyof RestEndpointMethodTypes[T]
> = (result: InferGhResponse<T, S>) => any;

type DefaultTransformer<
	T extends keyof RestEndpointMethodTypes,
	S extends keyof RestEndpointMethodTypes[T]
> = (result: InferGhResponse<T, S>) => InferGhResponse<T, S>;
/**
 * Effectively the `BaseQueryFn` we want, but located in this service file
 * until we figure out a way of making the generic parameters work.
 *
 * TODO: Figure out a way of doing this without @ts-expect-error?
 */
export async function ghQuery<
	T extends keyof RestEndpointMethodTypes,
	S extends keyof RestEndpointMethodTypes[T],
	U extends Transformer<T, S> | undefined = DefaultTransformer<T, S>
>(
	args: GhArgs<T, S>,
	transform?: U
): Promise<U extends Transformer<T, S> ? ReturnType<U> : InferGhResponse<T, S>> {
	if (!hasGitHub(args.extra)) throw new Error('No GitHub client!');
	const gh = args.extra.gitHubClient;
	try {
		// @ts-expect-error because of dynamic keys.
		const result = await gh.octokit[args.domain][args.action]({
			...args.parameters,
			headers: DEFAULT_HEADERS,
			owner: gh.owner,
			repo: gh.repo
		});
		return { data: transform ? transform(result.data) : result.data } as any;
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
