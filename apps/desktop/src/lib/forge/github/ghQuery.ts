import { Code } from '$lib/error/knownErrors';
import { GitHubClient } from '$lib/forge/github/githubClient';
import { DEFAULT_HEADERS } from '$lib/forge/github/headers';
import { isErrorlike } from '@gitbutler/ui/utils/typeguards';
import type { Prettify } from '@gitbutler/shared/utils/typeUtils';
import type { Octokit } from '@octokit/rest';

type OwnerAndRepo = { owner: string; repo: string };

type DomainKey = keyof Octokit;

type StringFunctionKeys<T> = {
	[K in keyof T]: T[K] extends (...args: any[]) => any ? (K extends string ? K : never) : never;
}[keyof T];

type DomainAction<K extends DomainKey> = StringFunctionKeys<Octokit[K]>;

type ActionType<KB extends DomainKey, A extends DomainAction<KB>> = Octokit[KB][A];

type DomainActionParameters<D extends DomainKey, A extends DomainAction<D>> =
	ActionType<D, A> extends (...args: any[]) => any
		? Prettify<Parameters<ActionType<D, A>>[0]>
		: never;

type DomainActionResponse<D extends DomainKey, A extends DomainAction<D>> =
	ActionType<D, A> extends (...args: any[]) => Promise<infer R>
		? R extends { data: infer Data }
			? Data
			: never
		: never;

type GhQueryArgs<
	T extends DomainKey,
	S extends DomainAction<T>,
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
 */
export async function ghQuery<
	T extends DomainKey,
	S extends DomainAction<T>,
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
		if (typeof args === 'function') {
			if ((!gh.owner || !gh.repo) && requiresOwnerAndRepo === 'required') {
				throw { name: 'GitHub API error', message: 'No GitHub owner or repo!' };
			}
			return await args(gh.octokit, {
				owner: gh.owner,
				repo: gh.repo
			} as RequiresOwnerAndRepo extends 'required' ? OwnerAndRepo : Partial<OwnerAndRepo>);
		}

		const action = gh.octokit[args.domain][args.action];
		if (typeof action === 'function') {
			return await action({
				...args.parameters,
				headers: DEFAULT_HEADERS,
				owner: gh.owner,
				repo: gh.repo
			});
		}

		throw new Error(`Invalid action: ${args.domain}/${args.action}`);
	} catch (err: unknown) {
		const argInfo = extractDomainAndAction<T, S, RequiresOwnerAndRepo>(args);
		const title = argInfo
			? `GitHub API error: ${argInfo.domain}/${argInfo.action}`
			: 'GitHub API error';

		const message = isErrorlike(err) ? err.message : String(err);

		// Check for stacked PR across forks error (base field invalid)
		let code: string | undefined;
		if (isErrorlike(err) && 'response' in err) {
			const response = (err as any).response;
			if (response?.data?.errors instanceof Array) {
				const hasInvalidBaseError = response.data.errors.some(
					(error: any) =>
						error.resource === 'PullRequest' && error.field === 'base' && error.code === 'invalid'
				);
				if (hasInvalidBaseError) {
					code = Code.GitHubStackedPrFork;
				}
			}
		}

		// Check for expired token
		if (!code && message.startsWith('Not Found -')) {
			code = Code.GitHubTokenExpired;
		}

		return { error: { name: title, message, code } };
	}
}

/**
 * Argument to `ghQuery` where parameters are inferred from octokit.js.
 */
type GhArgs<T extends DomainKey, S extends DomainAction<T>> = {
	domain: T;
	action: S;
	parameters?: Omit<DomainActionParameters<T, S>, 'owner' | 'string'>;
	extra: unknown;
};

export type GhResponse<T> =
	| { data: T; error?: never }
	| { error: { name: string; message: string; code?: string }; data?: never };
/**
 * Response type for `ghQuery` inferred from octokit.js.
 */
type InferGhResponse<T extends DomainKey, S extends DomainAction<T>> = GhResponse<
	DomainActionResponse<T, S>
>;

function extractExtraParameters<
	T extends DomainKey,
	S extends DomainAction<T>,
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
	T extends DomainKey,
	S extends DomainAction<T>,
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
