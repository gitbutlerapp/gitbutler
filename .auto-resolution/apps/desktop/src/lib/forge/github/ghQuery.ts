import { Code } from '$lib/error/knownErrors';
import { GitHubClient } from '$lib/forge/github/githubClient';
import { DEFAULT_HEADERS } from '$lib/forge/github/headers';
import { isErrorlike } from '@gitbutler/ui/utils/typeguards';
import type { Octokit, RestEndpointMethodTypes } from '@octokit/rest';

type OwnerAndRepo = { owner: string; repo: string };

type GhQueryArgs<
	T extends keyof RestEndpointMethodTypes,
	S extends keyof RestEndpointMethodTypes[T],
	RequiresOwnerAndRepo extends 'required' | 'optional'
> =
	| GhArgs<T, S>
	| ((
			octokit: Octokit,
			about: RequiresOwnerAndRepo extends 'required' ? OwnerAndRepo : Partial<OwnerAndRepo>
	  ) => Promise<InferGhResponse<T, S>>);

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
	args: GhQueryArgs<T, S, RequiresOwnerAndRepo>,
	passedExtra?: unknown,
	requiresOwnerAndRepo?: RequiresOwnerAndRepo
): Promise<InferGhResponse<T, S>> {
	const extra = extractExtraParameters<T, S, RequiresOwnerAndRepo>(args, passedExtra);

	if (!hasGitHub(extra)) throw new Error('No GitHub client!');
	const gh = extra.gitHubClient;
	try {
		let result;
		if (typeof args === 'function') {
			if ((!gh.owner || !gh.repo) && requiresOwnerAndRepo === 'required') {
				throw { name: 'GitHub API error', message: 'No GitHub owner or repo!' };
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
		const argInfo = extractDomainAndAction<T, S, RequiresOwnerAndRepo>(args);
		const title = argInfo
			? `GitHub API error: ${argInfo.domain}/${argInfo.action}`
			: 'GitHub API error';

		const message = isErrorlike(err) ? err.message : String(err);
		const code = message.startsWith('Not Found -') ? Code.GitHubTokenExpired : undefined;

		return { error: { name: title, message, code } };
	}
}

/**
 * Argument to `ghQuery` where parameters are inferred from octokit.js.
 */
type GhArgs<
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

export type GhResponse<T> =
	| { data: T; error?: never }
	| { error: { name: string; message: string; code?: string }; data?: never };
/**
 * Response type for `ghQuery` inferred from octokit.js.
 */
type InferGhResponse<
	T extends keyof RestEndpointMethodTypes,
	S extends keyof RestEndpointMethodTypes[T],
	O extends keyof RestEndpointMethodTypes[T][S] = keyof RestEndpointMethodTypes[T][S] & 'response',
	Q extends keyof RestEndpointMethodTypes[T][S][O] = keyof RestEndpointMethodTypes[T][S][O] &
		'data',
	P extends RestEndpointMethodTypes[T][S][O][Q] = RestEndpointMethodTypes[T][S][O][Q]
> = GhResponse<P>;

function extractExtraParameters<
	T extends keyof RestEndpointMethodTypes,
	S extends keyof RestEndpointMethodTypes[T],
	RequiresOwnerAndRepo extends 'required' | 'optional'
>(args: GhQueryArgs<T, S, RequiresOwnerAndRepo>, passedExtra: unknown) {
	let extra;
	if (typeof args === 'function') {
		extra = passedExtra;
	} else {
		extra = args.extra;
	}
	return extra;
}

function extractDomainAndAction<
	T extends keyof RestEndpointMethodTypes,
	S extends keyof RestEndpointMethodTypes[T],
	RequiresOwnerAndRepo extends 'required' | 'optional'
>(args: GhQueryArgs<T, S, RequiresOwnerAndRepo>): { domain: string; action: string } | undefined {
	if (typeof args !== 'function') {
		return { domain: args.domain, action: String(args.action) };
	}
	return undefined;
}

/**
 * Typeguard for accessing injected `GitHubClient` dependency safely.
 */
function hasGitHub(extra: unknown): extra is {
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
