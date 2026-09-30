<script lang="ts">
	import BitbucketAccountBadge from "$components/forge/BitbucketAccountBadge.svelte";
	import GitHubAccountBadge from "$components/forge/GitHubAccountBadge.svelte";
	import GitLabAccountBadge from "$components/forge/GitLabAccountBadge.svelte";
	import GiteeAccountBadge from "$components/forge/GiteeAccountBadge.svelte";
	import ForgeAccountConfig from "$components/projectSettings/ForgeAccountConfig.svelte";
	import GitHubOrgRestrictionNotice from "$components/projectSettings/GitHubOrgRestrictionNotice.svelte";
	import { GIT_CONFIG_SERVICE } from "$lib/config/gitConfigService";
	import { isNormalizedError } from "$lib/error/normalizedError";
	import {
		bitbucketAccountIdentifierToString,
		stringToBitbucketAccountIdentifier,
	} from "$lib/forge/bitbucket/bitbucketUserService.svelte";
	import { usePreferredBitbucketUsername } from "$lib/forge/bitbucket/hooks.svelte";
	import { FORGE_INFO_SERVICE } from "$lib/forge/forgeInfo.svelte";
	import {
		giteeAccountIdentifierToString,
		stringToGiteeAccountIdentifier,
	} from "$lib/forge/gitee/giteeUserService.svelte";
	import { usePreferredGiteeUsername } from "$lib/forge/gitee/hooks.svelte";
	import {
		githubAccountIdentifierToString,
		stringToGitHubAccountIdentifier,
	} from "$lib/forge/github/githubUserService.svelte";
	import { usePreferredGitHubUsername } from "$lib/forge/github/hooks.svelte";
	import {
		gitlabAccountIdentifierToString,
		stringToGitLabAccountIdentifier,
	} from "$lib/forge/gitlab/gitlabUserService.svelte";
	import { usePreferredGitLabUsername } from "$lib/forge/gitlab/hooks.svelte";
	import { LISTING_SERVICE } from "$lib/forge/listingService.svelte";
	import { t } from "$lib/i18n";
	import { PROJECTS_SERVICE } from "$lib/project/projectsService";
	import { inject } from "@gitbutler/core/context";
	import { reactive } from "@gitbutler/shared/reactiveUtils.svelte";
	import { CardGroup, Select, SelectItem } from "@gitbutler/ui-svelte";

	import type { Project } from "$lib/project/project";
	import type {
		BitbucketAccountIdentifier,
		ForgeName,
		ForgeUser,
		GiteeAccountIdentifier,
		GitHubStackingMode,
		GithubAccountIdentifier,
		GitlabAccountIdentifier,
		ReviewStackingDescription,
	} from "@gitbutler/but-sdk";

	type ForgeSelection = ForgeName | "default";

	const FORGE_OPTIONS: { label: string; value: ForgeSelection }[] = [
		{ label: "None", value: "default" },
		{ label: "GitHub", value: "github" },
		{ label: "GitLab", value: "gitlab" },
		{ label: "Azure", value: "azure" },
		{ label: "BitBucket", value: "bitbucket" },
		{ label: "Gitee", value: "gitee" },
	];

	const { projectId }: { projectId: string } = $props();

	const forgeInfoService = inject(FORGE_INFO_SERVICE);
	const forgeInfoQuery = $derived(forgeInfoService.get(projectId));
	const forgeInfo = $derived(forgeInfoQuery.response);
	const determinedForgeType = $derived(forgeInfo?.name ?? "default");
	const projectsService = inject(PROJECTS_SERVICE);
	const gitConfigService = inject(GIT_CONFIG_SERVICE);
	const gitConfigQuery = $derived(gitConfigService.gbConfig(projectId));
	const reviewStackingDescription = $derived(
		(gitConfigQuery.response?.gitbutlerReviewStackingDescription ??
			"bottom") as ReviewStackingDescription,
	);
	const githubStackingMode = $derived(
		(gitConfigQuery.response?.gitbutlerGithubStackingMode ?? "auto") as GitHubStackingMode,
	);
	const projectQuery = $derived(projectsService.getProject(projectId));
	const project = $derived(projectQuery.response);

	const selectedOption = $derived(project?.forge_override || "default");

	// GitHub hooks
	const { preferredGitHubAccount, githubAccounts } = usePreferredGitHubUsername(
		reactive(() => projectId),
	);

	// The workspace's polled review listing keeps this cache entry current,
	// so it reflects whether the integration currently works at all.
	const listingService = inject(LISTING_SERVICE);
	const listingState = $derived(listingService.listingState(projectId));
	const listingError = $derived(listingState.result.error);
	const listingErrorCode = $derived(
		isNormalizedError(listingError) ? listingError.code : undefined,
	);

	// GitLab hooks
	const { preferredGitLabAccount, gitlabAccounts } = usePreferredGitLabUsername(
		reactive(() => projectId),
	);

	// Bitbucket hooks
	const { preferredBitbucketAccount, bitbucketAccounts } = usePreferredBitbucketUsername(
		reactive(() => projectId),
	);

	// Gitee hooks
	const { preferredGiteeAccount, giteeAccounts } = usePreferredGiteeUsername(
		reactive(() => projectId),
	);

	function handleSelectionChange(selectedOption: ForgeSelection) {
		if (!project) return;

		const mutableProject: Project & { unset_forge_override?: boolean } = structuredClone(project);

		if (selectedOption === "default") {
			mutableProject.unset_forge_override = true;
		} else {
			mutableProject.forge_override = selectedOption;
		}
		projectsService.updateProject(mutableProject);
	}

	async function updatePreferredForgeUser(projectId: string, forgeUser: ForgeUser) {
		await projectsService.updatePreferredForgeUser(projectId, forgeUser);
		// The cached review listing was fetched with the previous account's
		// credentials; refresh it so a credential-caused failure (e.g. an
		// org OAuth restriction) clears as soon as the account changes
		// instead of on the next 15-minute poll.
		await listingService.refresh(projectId);
	}

	async function updatePreferredGitHubAccount(projectId: string, account: GithubAccountIdentifier) {
		await updatePreferredForgeUser(projectId, { provider: "github", details: account });
	}

	async function updatePreferredGitLabAccount(projectId: string, account: GitlabAccountIdentifier) {
		await updatePreferredForgeUser(projectId, { provider: "gitlab", details: account });
	}

	async function updatePreferredBitbucketAccount(
		projectId: string,
		account: BitbucketAccountIdentifier,
	) {
		await updatePreferredForgeUser(projectId, { provider: "bitbucket", details: account });
	}

	async function updatePreferredGiteeAccount(projectId: string, account: GiteeAccountIdentifier) {
		await updatePreferredForgeUser(projectId, { provider: "gitee", details: account });
	}

	async function updateReviewStackingDescription(value: ReviewStackingDescription) {
		await gitConfigService.setGbConfig(projectId, { gitbutlerReviewStackingDescription: value });
	}

	async function updateGitHubStackingMode(value: GitHubStackingMode) {
		await gitConfigService.setGbConfig(projectId, { gitbutlerGithubStackingMode: value });
	}
</script>

<CardGroup>
	<CardGroup.Item>
		{#snippet title()}
			{t("forge-override")}
		{/snippet}

		{#snippet caption()}
			{#if determinedForgeType === "default"}
				{t("we-couldn-t-detect-which-forge-you-re-using")}
				<br />
				{t("to-enable-forge-integration-please-select-your-forge-from-th")}
				<br />
				<span class="text-bold">Note:</span>
				{t("currently-only-github-gitlab-and-bitbucket-support-pull-requ")}
			{:else}
				{t("we-ve-detected-that-you-re-using")}
				<span class="text-bold">{determinedForgeType.toUpperCase()}</span>.
				<br />
				{t("at-the-moment-it-s-not-possible-to-manually-override-the-det")}
			{/if}
		{/snippet}

		{#if determinedForgeType === "default"}
			<Select
				value={selectedOption}
				options={FORGE_OPTIONS}
				wide
				onselect={(value) => handleSelectionChange(value as ForgeSelection)}
			>
				{#snippet itemSnippet({ item, highlighted })}
					<SelectItem selected={item.value === selectedOption} {highlighted}>
						{item.label}
					</SelectItem>
				{/snippet}
			</Select>
		{/if}
	</CardGroup.Item>

	<CardGroup.Item>
		{#snippet title()}
			{t("stack-information-in-review-descriptions")}
		{/snippet}

		{#snippet caption()}
			{t("choose-where-gitbutler-managed-stack-information-appears-cha")}
		{/snippet}

		<div data-testid="review-stacking-description-select">
			<Select
				value={reviewStackingDescription}
				options={[
					{ label: "Bottom", value: "bottom" },
					{ label: "Top", value: "top" },
					{ label: "Disabled", value: "disabled" },
				]}
				wide
				onselect={(value) => updateReviewStackingDescription(value as ReviewStackingDescription)}
			>
				{#snippet itemSnippet({ item, highlighted })}
					<div data-testid={`review-stacking-description-option-${item.value}`}>
						<SelectItem selected={item.value === reviewStackingDescription} {highlighted}>
							{item.label}
						</SelectItem>
					</div>
				{/snippet}
			</Select>
		</div>
	</CardGroup.Item>

	{#if forgeInfo?.name === "github"}
		<CardGroup.Item>
			{#snippet title()}
				{t("native-github-stacked-pull-requests")}
			{/snippet}

			{#snippet caption()}
				Register this project’s reviewed stacks with GitHub’s private-preview stacks API. Higher
				pull requests may merge the pull requests below them. Auto falls back to description
				metadata when the repository is not enrolled in the preview, while Native reports an error.
				Changes apply on the next push or pull request creation. Fork-backed pull requests always
				use description metadata.
			{/snippet}

			<div data-testid="github-stacking-mode-select">
				<Select
					value={githubStackingMode}
					options={[
						{ label: "Auto", value: "auto" },
						{ label: "Disabled", value: "disabled" },
						{ label: "Native", value: "native" },
					]}
					wide
					onselect={(value) => updateGitHubStackingMode(value as GitHubStackingMode)}
				>
					{#snippet itemSnippet({ item, highlighted })}
						<div data-testid={`github-stacking-mode-option-${item.value}`}>
							<SelectItem selected={item.value === githubStackingMode} {highlighted}>
								{item.label}
							</SelectItem>
						</div>
					{/snippet}
				</Select>
			</div>
		</CardGroup.Item>

		<ForgeAccountConfig
			{projectId}
			displayName="GitHub"
			accounts={githubAccounts.current}
			preferredAccount={preferredGitHubAccount.current}
			accountToString={githubAccountIdentifierToString}
			stringToAccount={stringToGitHubAccountIdentifier}
			getUsername={(account) => account.info.username}
			updatePreferredAccount={updatePreferredGitHubAccount}
			AccountBadge={GitHubAccountBadge}
			docsUrl="https://docs.gitbutler.com/features/forge-integration/github-integration"
			requestType="pull request"
		>
			{#snippet notice()}
				<GitHubOrgRestrictionNotice errorCode={listingErrorCode} />
			{/snippet}
		</ForgeAccountConfig>
	{/if}

	{#if forgeInfo?.name === "gitlab"}
		<ForgeAccountConfig
			{projectId}
			displayName="GitLab"
			accounts={gitlabAccounts.current}
			preferredAccount={preferredGitLabAccount.current}
			accountToString={gitlabAccountIdentifierToString}
			stringToAccount={stringToGitLabAccountIdentifier}
			getUsername={(account) => account.info.username}
			updatePreferredAccount={updatePreferredGitLabAccount}
			AccountBadge={GitLabAccountBadge}
			docsUrl="https://docs.gitbutler.com/features/forge-integration/gitlab-integration"
			requestType="merge request"
		/>
	{/if}

	{#if forgeInfo?.name === "bitbucket"}
		<ForgeAccountConfig
			{projectId}
			displayName="Bitbucket"
			accounts={bitbucketAccounts.current}
			preferredAccount={preferredBitbucketAccount.current}
			accountToString={bitbucketAccountIdentifierToString}
			stringToAccount={stringToBitbucketAccountIdentifier}
			getUsername={(account) => account.info.email}
			updatePreferredAccount={updatePreferredBitbucketAccount}
			AccountBadge={BitbucketAccountBadge}
			docsUrl="https://docs.gitbutler.com/features/forge-integration/bitbucket-integration"
			requestType="pull request"
		/>
	{/if}

	{#if forgeInfo?.name === "gitee"}
		<ForgeAccountConfig
			{projectId}
			displayName="Gitee"
			accounts={giteeAccounts.current}
			preferredAccount={preferredGiteeAccount.current}
			accountToString={giteeAccountIdentifierToString}
			stringToAccount={stringToGiteeAccountIdentifier}
			getUsername={(account) => account.info.username}
			updatePreferredAccount={updatePreferredGiteeAccount}
			AccountBadge={GiteeAccountBadge}
			docsUrl="https://gitee.com/profile/personal_access_tokens"
			requestType="pull request"
		/>
	{/if}
</CardGroup>
