<script lang="ts">
	import BitbucketAccountBadge from "$components/forge/BitbucketAccountBadge.svelte";
	import GitHubAccountBadge from "$components/forge/GitHubAccountBadge.svelte";
	import GitLabAccountBadge from "$components/forge/GitLabAccountBadge.svelte";
	import ForgeAccountConfig from "$components/projectSettings/ForgeAccountConfig.svelte";
	import { GIT_CONFIG_SERVICE } from "$lib/config/gitConfigService";
	import {
		bitbucketAccountIdentifierToString,
		stringToBitbucketAccountIdentifier,
	} from "$lib/forge/bitbucket/bitbucketUserService.svelte";
	import { usePreferredBitbucketUsername } from "$lib/forge/bitbucket/hooks.svelte";
	import { FORGE_INFO_SERVICE } from "$lib/forge/forgeInfo.svelte";
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
	import { PROJECTS_SERVICE } from "$lib/project/projectsService";
	import { inject } from "@gitbutler/core/context";
	import { reactive } from "@gitbutler/shared/reactiveUtils.svelte";
	import { CardGroup, Select, SelectItem } from "@gitbutler/ui";

	import type { Project } from "$lib/project/project";
	import type {
		BitbucketAccountIdentifier,
		ForgeName,
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

	// GitLab hooks
	const { preferredGitLabAccount, gitlabAccounts } = usePreferredGitLabUsername(
		reactive(() => projectId),
	);

	// Bitbucket hooks
	const { preferredBitbucketAccount, bitbucketAccounts } = usePreferredBitbucketUsername(
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

	function updatePreferredGitHubAccount(projectId: string, account: GithubAccountIdentifier) {
		projectsService.updatePreferredForgeUser(projectId, {
			provider: "github",
			details: account,
		});
	}

	function updatePreferredGitLabAccount(projectId: string, account: GitlabAccountIdentifier) {
		projectsService.updatePreferredForgeUser(projectId, {
			provider: "gitlab",
			details: account,
		});
	}

	function updatePreferredBitbucketAccount(projectId: string, account: BitbucketAccountIdentifier) {
		projectsService.updatePreferredForgeUser(projectId, {
			provider: "bitbucket",
			details: account,
		});
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
			Forge override
		{/snippet}

		{#snippet caption()}
			{#if determinedForgeType === "default"}
				We couldn't detect which Forge you're using.
				<br />
				To enable Forge integration, please select your Forge from the dropdown below.
				<br />
				<span class="text-bold">Note:</span> Currently, only GitHub, GitLab and Bitbucket support pull
				request creation.
			{:else}
				We’ve detected that you’re using <span class="text-bold"
					>{determinedForgeType.toUpperCase()}</span
				>.
				<br />
				At the moment, it’s not possible to manually override the detected forge type.
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
			Stack information in review descriptions
		{/snippet}

		{#snippet caption()}
			Choose where GitButler-managed stack information appears. Changes apply on the next review
			sync. The default is Bottom.
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
				Native GitHub stacked pull requests
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
		/>
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
</CardGroup>
