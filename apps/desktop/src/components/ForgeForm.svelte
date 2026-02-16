<script lang="ts">
	import ForgeAccountConfig from "$components/ForgeAccountConfig.svelte";
	import GitHubAccountBadge from "$components/GitHubAccountBadge.svelte";
	import GitLabAccountBadge from "$components/GitLabAccountBadge.svelte";
	import { DEFAULT_FORGE_FACTORY } from "$lib/forge/forgeFactory.svelte";
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

	import type { ForgeName } from "$lib/forge/interface/forge";
	import type { Project } from "$lib/project/project";
	import type { ButGitHubToken, ButGitLabToken } from "@gitbutler/core/api";

	const FORGE_OPTIONS: { label: string; value: ForgeName }[] = [
		{ label: "None", value: "default" },
		{ label: "GitHub", value: "github" },
		{ label: "GitLab", value: "gitlab" },
		{ label: "Azure", value: "azure" },
		{ label: "BitBucket", value: "bitbucket" },
	];

	const { projectId }: { projectId: string } = $props();

	const forge = inject(DEFAULT_FORGE_FACTORY);
	const projectsService = inject(PROJECTS_SERVICE);
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

	function handleSelectionChange(selectedOption: ForgeName) {
		if (!project) return;

		const mutableProject: Project & { unset_forge_override?: boolean } = structuredClone(project);

		if (selectedOption === "default") {
			mutableProject.unset_forge_override = true;
		} else {
			mutableProject.forge_override = selectedOption;
		}
		projectsService.updateProject(mutableProject);
	}

	function updatePreferredGitHubAccount(
		projectId: string,
		account: ButGitHubToken.GithubAccountIdentifier,
	) {
		projectsService.updatePreferredForgeUser(projectId, {
			provider: "github",
			details: account,
		});
	}

	function updatePreferredGitLabAccount(
		projectId: string,
		account: ButGitLabToken.GitlabAccountIdentifier,
	) {
		projectsService.updatePreferredForgeUser(projectId, {
			provider: "gitlab",
			details: account,
		});
	}
</script>

<CardGroup>
	<CardGroup.Item>
		{#snippet title()}
			Forge override
		{/snippet}

		{#snippet caption()}
			{#if forge.determinedForgeType === "default"}
				We couldn't detect which Forge you're using.
				<br />
				To enable Forge integration, please select your Forge from the dropdown below.
				<br />
				<span class="text-bold">Note:</span> Currently, only GitHub and GitLab support pull request creation.
			{:else}
				We’ve detected that you’re using <span class="text-bold"
					>{forge.determinedForgeType.toUpperCase()}</span
				>.
				<br />
				At the moment, it’s not possible to manually override the detected forge type.
			{/if}
		{/snippet}

		{#if forge.determinedForgeType === "default"}
			<Select
				value={selectedOption}
				options={FORGE_OPTIONS}
				wide
				onselect={(value) => handleSelectionChange(value as ForgeName)}
			>
				{#snippet itemSnippet({ item, highlighted })}
					<SelectItem selected={item.value === selectedOption} {highlighted}>
						{item.label}
					</SelectItem>
				{/snippet}
			</Select>
		{/if}
	</CardGroup.Item>

	{#if forge.current.name === "github"}
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

	{#if forge.current.name === "gitlab"}
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
</CardGroup>
