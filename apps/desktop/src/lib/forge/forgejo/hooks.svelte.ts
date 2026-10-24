// forgejo-fork: mirrors the Bitbucket hooks.
import {
	FORGEJO_USER_SERVICE,
	isSameForgejoAccountIdentifier,
} from "$lib/forge/forgejo/forgejoUserService.svelte";
import { PROJECTS_SERVICE } from "$lib/project/projectsService";
import { inject } from "@gitbutler/core/context";
import { reactive } from "@gitbutler/shared/reactiveUtils.svelte";
import type { ForgeUserQuery } from "$lib/forge/interface/types";
import type { ForgejoAccountIdentifier } from "@gitbutler/but-sdk";
import type { Reactive } from "@gitbutler/shared/storeUtils";

type ForgejoPreferences = {
	preferredForgejoAccount: Reactive<ForgejoAccountIdentifier | undefined>;
	forgejoAccounts: Reactive<ForgejoAccountIdentifier[]>;
};

/**
 * Return the preferred Forgejo account for the given project ID: the project's
 * preferred forge user when it is a known Forgejo account, else the first one.
 */
export function usePreferredForgejoAccount(projectId: Reactive<string>): ForgejoPreferences {
	const forgejoUserService = inject(FORGEJO_USER_SERVICE);
	const projectsService = inject(PROJECTS_SERVICE);
	const accountsResponse = forgejoUserService.accounts();
	const forgejoAccounts = $derived(accountsResponse?.response ?? []);

	const projectQuery = $derived(projectsService.getProject(projectId.current));
	const project = $derived(projectQuery.response);
	const preferredUser = $derived.by(() => {
		const preferred = project?.preferred_forge_user;
		if (preferred?.provider !== "forgejo") {
			return forgejoAccounts.at(0);
		}
		return (
			forgejoAccounts.find((account) =>
				isSameForgejoAccountIdentifier(account, preferred.details),
			) ?? forgejoAccounts.at(0)
		);
	});

	return {
		preferredForgejoAccount: reactive(() => preferredUser),
		forgejoAccounts: reactive(() => forgejoAccounts),
	};
}

/**
 * Resolve the project's preferred Forgejo account and fetch it as a
 * display-ready `ForgeUser`. `user` is `undefined` when no Forgejo
 * account is configured.
 *
 * `inject()` runs once at call time, so this must be invoked during
 * component init — not inside a `$derived`.
 */
export function useForgejoForgeUser(projectId: Reactive<string>): ForgeUserQuery {
	const forgejoUserService = inject(FORGEJO_USER_SERVICE);
	const { preferredForgejoAccount } = usePreferredForgejoAccount(projectId);
	const userQuery = $derived.by(() => {
		const account = preferredForgejoAccount.current;
		if (account === undefined) return undefined;
		return forgejoUserService.authenticatedUser(account, {
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
