import { useBitbucketForgeUser } from "$lib/forge/bitbucket/hooks.svelte";
import { FORGE_INFO_SERVICE } from "$lib/forge/forgeInfo.svelte";
import { useGitHubForgeUser } from "$lib/forge/github/hooks.svelte";
import { useGitLabForgeUser } from "$lib/forge/gitlab/hooks.svelte";
import { inject } from "@gitbutler/core/context";
import { reactive } from "@gitbutler/shared/reactiveUtils.svelte";
import type { Reactive } from "@gitbutler/shared/storeUtils";

/**
 * Per-project forge auth state. `authenticated` is true when the
 * project's preferred forge user has a usable account configured.
 * For forges with no Rust-side user support (Azure), `authenticated`
 * is always false.
 */
export function useForgeAuth(projectId: Reactive<string>): {
	authenticated: Reactive<boolean>;
	isLoading: Reactive<boolean>;
} {
	const forgeInfoService = inject(FORGE_INFO_SERVICE);
	const forgeInfoQuery = $derived(forgeInfoService.get(projectId.current));
	const forgeName = $derived(forgeInfoQuery.response?.name);

	// These hooks call inject()/getContext(), which must run during
	// component init — never inside a $derived that re-runs in a reaction.
	// Call both unconditionally; each stays inert until its forge's
	// account resolves, so the non-active forge costs nothing.
	const githubUser = useGitHubForgeUser(projectId);
	const gitlabUser = useGitLabForgeUser(projectId);
	const bitbucketUser = useBitbucketForgeUser(projectId);

	const authenticated = $derived(
		forgeName === "github"
			? githubUser.user.current !== undefined
			: forgeName === "gitlab"
				? gitlabUser.user.current !== undefined
				: forgeName === "bitbucket"
					? bitbucketUser.user.current !== undefined
					: false,
	);
	const isLoading = $derived(
		forgeName === "github"
			? githubUser.isLoading.current
			: forgeName === "gitlab"
				? gitlabUser.isLoading.current
				: forgeName === "bitbucket"
					? bitbucketUser.isLoading.current
					: false,
	);

	return {
		authenticated: reactive(() => authenticated),
		isLoading: reactive(() => isLoading),
	};
}
