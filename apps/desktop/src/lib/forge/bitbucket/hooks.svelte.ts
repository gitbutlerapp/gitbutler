import {
	BITBUCKET_USER_SERVICE,
	isSameBitbucketAccountIdentifier,
} from "$lib/forge/bitbucket/bitbucketUserService.svelte";
import { PROJECTS_SERVICE } from "$lib/project/projectsService";
import { inject } from "@gitbutler/core/context";
import { reactive } from "@gitbutler/shared/reactiveUtils.svelte";
import type { ForgeUserQuery } from "$lib/forge/interface/types";
import type { BitbucketAccountIdentifier } from "@gitbutler/but-sdk";
import type { Reactive } from "@gitbutler/shared/storeUtils";

type BitbucketPreferences = {
	preferredBitbucketAccount: Reactive<BitbucketAccountIdentifier | undefined>;
	bitbucketAccounts: Reactive<BitbucketAccountIdentifier[]>;
};

/**
 * Return the preferred Bitbucket account for the given project ID, based on the
 * known Bitbucket accounts in the application settings and the project's
 * preferred forge user.
 */
export function usePreferredBitbucketUsername(projectId: Reactive<string>): BitbucketPreferences {
	const bitbucketUserService = inject(BITBUCKET_USER_SERVICE);
	const projectsService = inject(PROJECTS_SERVICE);
	const bitbucketAccountsResponse = bitbucketUserService.accounts();
	const bitbucketAccounts = $derived(bitbucketAccountsResponse?.response ?? []);

	const projectQuery = $derived(projectsService.getProject(projectId.current));
	const project = $derived(projectQuery.response);
	const preferredUser = $derived.by(() => {
		if (bitbucketAccounts.length === 0) {
			return undefined;
		}

		if (
			project === undefined ||
			project.preferred_forge_user === null ||
			project.preferred_forge_user.provider !== "bitbucket"
		) {
			return bitbucketAccounts.at(0);
		}

		const preferredForgeUser = project.preferred_forge_user.details;

		return (
			bitbucketAccounts.find((account) =>
				isSameBitbucketAccountIdentifier(account, preferredForgeUser),
			) ?? bitbucketAccounts.at(0)
		);
	});

	return {
		preferredBitbucketAccount: reactive(() => preferredUser),
		bitbucketAccounts: reactive(() => bitbucketAccounts),
	};
}

/**
 * Resolve the project's preferred Bitbucket account and fetch it as a
 * display-ready `ForgeUser`. `user` is `undefined` when no Bitbucket
 * account is configured.
 *
 * `inject()` runs once at call time, so this must be invoked during
 * component init — not inside a `$derived`. The account lookup and the
 * user query are built reactively inside, so the result still updates
 * when the preferred account resolves.
 */
export function useBitbucketForgeUser(projectId: Reactive<string>): ForgeUserQuery {
	const bitbucketUserService = inject(BITBUCKET_USER_SERVICE);
	const { preferredBitbucketAccount } = usePreferredBitbucketUsername(projectId);
	const userQuery = $derived.by(() => {
		const account = preferredBitbucketAccount.current;
		if (account === undefined) return undefined;
		return bitbucketUserService.authenticatedUser(account, {
			transform: (result) =>
				result
					? {
							login: result.username,
							name: result.name ?? result.username,
							srcUrl: result.avatarUrl ?? "",
						}
					: undefined,
		});
	});
	return {
		user: reactive(() => userQuery?.response),
		isLoading: reactive(() => userQuery?.result.isLoading ?? false),
	};
}
