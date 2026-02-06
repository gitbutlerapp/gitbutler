<script lang="ts">
	import GitHubAccountBadge from '$components/GitHubAccountBadge.svelte';
	import GitLabAccountBadge from '$components/GitLabAccountBadge.svelte';
	import { DEFAULT_FORGE_FACTORY } from '$lib/forge/forgeFactory.svelte';
	import {
		githubAccountIdentifierToString,
		stringToGitHubAccountIdentifier
	} from '$lib/forge/github/githubUserService.svelte';
	import { usePreferredGitHubUsername } from '$lib/forge/github/hooks.svelte';
	import {
		gitlabAccountIdentifierToString,
		stringToGitLabAccountIdentifier
	} from '$lib/forge/gitlab/gitlabUserService.svelte';
	import { usePreferredGitLabUsername } from '$lib/forge/gitlab/hooks.svelte';
	import { PROJECTS_SERVICE } from '$lib/project/projectsService';
	import { useSettingsModal } from '$lib/settings/settingsModal.svelte';
	import { inject } from '@gitbutler/core/context';
	import { reactive } from '@gitbutler/shared/reactiveUtils.svelte';
	import { Button, CardGroup, InfoMessage, Link, Select, SelectItem } from '@gitbutler/ui';

	import type { ForgeName } from '$lib/forge/interface/forge';
	import type { Project } from '$lib/project/project';

	const { projectId }: { projectId: string } = $props();

	const forge = inject(DEFAULT_FORGE_FACTORY);
	const { preferredGitHubAccount, githubAccounts } = usePreferredGitHubUsername(
		reactive(() => projectId)
	);
	const { preferredGitLabAccount, gitlabAccounts } = usePreferredGitLabUsername(
		reactive(() => projectId)
	);

	const { openGeneralSettings } = useSettingsModal();

	const projectsService = inject(PROJECTS_SERVICE);
	const projectQuery = $derived(projectsService.getProject(projectId));
	const project = $derived(projectQuery.response);

	const forgeOptions: { label: string; value: ForgeName }[] = [
		{
			label: 'None',
			value: 'default'
		},
		{
			label: 'GitHub',
			value: 'github'
		},
		{
			label: 'GitLab',
			value: 'gitlab'
		},
		{
			label: 'Azure',
			value: 'azure'
		},
		{
			label: 'BitBucket',
			value: 'bitbucket'
		}
	];
	let selectedOption = $derived(project?.forge_override || 'default');

	function handleSelectionChange(selectedOption: ForgeName) {
		if (!project) return;

		const mutableProject: Project & { unset_forge_override?: boolean } = structuredClone(project);

		if (selectedOption === 'default') {
			mutableProject.unset_forge_override = true;
		} else {
			mutableProject.forge_override = selectedOption;
		}
		projectsService.updateProject(mutableProject);
	}
</script>

<CardGroup>
	<CardGroup.Item>
		{#snippet title()}
			Forge override
		{/snippet}

		{#snippet caption()}
			{#if forge.determinedForgeType === 'default'}
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

		{#if forge.determinedForgeType === 'default'}
			<Select
				value={selectedOption}
				options={forgeOptions}
				wide
				onselect={(value) => {
					selectedOption = value as ForgeName;
					handleSelectionChange(selectedOption);
				}}
			>
				{#snippet itemSnippet({ item, highlighted })}
					<SelectItem selected={item.value === selectedOption} {highlighted}>
						{item.label}
					</SelectItem>
				{/snippet}
			</Select>
		{/if}
	</CardGroup.Item>

	{#if forge.current.name === 'gitlab'}
		<CardGroup.Item>
			{#snippet title()}
				Configure GitLab integration
			{/snippet}

			{#snippet caption()}
				Enable merge request creation. Read more in the <Link
					href="https://docs.gitbutler.com/features/forge-integration/gitlab-integration">docs</Link
				>
			{/snippet}
			{#if gitlabAccounts.current.length === 0 || !preferredGitLabAccount.current}
				<InfoMessage style="warning" filled outlined={false}>
					{#snippet title()}
						No GitLab accounts found
					{/snippet}
					{#snippet content()}
						Add a GitLab account in General Settings to enable GitLab integration
					{/snippet}
				</InfoMessage>
				{@render openSettingsButton()}
			{:else}
				{@const account = preferredGitLabAccount.current}
				<Select
					label="GitLab account for this project"
					value={gitlabAccountIdentifierToString(account)}
					options={gitlabAccounts.current.map((account) => ({
						label: account.info.username,
						value: gitlabAccountIdentifierToString(account)
					}))}
					onselect={(value) => {
						const account = stringToGitLabAccountIdentifier(value);
						if (!account) return;
						projectsService.updatePreferredForgeUser(projectId, {
							provider: 'gitlab',
							details: account
						});
					}}
					disabled={gitlabAccounts.current.length <= 1}
					wide
				>
					{#snippet itemSnippet({ item, highlighted })}
						{@const itemAccount = item.value && stringToGitLabAccountIdentifier(item.value)}
						<SelectItem
							selected={item.value === gitlabAccountIdentifierToString(account)}
							{highlighted}
						>
							{item.label}

							{#if itemAccount}
								<GitLabAccountBadge account={itemAccount} class="m-l-4" />
							{/if}
						</SelectItem>
					{/snippet}
				</Select>
			{/if}
		</CardGroup.Item>
	{/if}

	{#if forge.current.name === 'github'}
		<CardGroup.Item>
			{#snippet title()}
				Configure GitHub integration
			{/snippet}

			{#snippet caption()}
				Enable pull request creation. Read more in the <Link
					href="https://docs.gitbutler.com/features/forge-integration/github-integration">docs</Link
				>
			{/snippet}

			{#if githubAccounts.current.length === 0 || !preferredGitHubAccount.current}
				<InfoMessage style="warning" filled outlined={false}>
					{#snippet title()}
						No GitHub accounts found
					{/snippet}
					{#snippet content()}
						Add a GitHub account in General Settings to enable GitHub integration
					{/snippet}
				</InfoMessage>
				{@render openSettingsButton()}
			{:else}
				{@const account = preferredGitHubAccount.current}
				<Select
					label="GitHub account for this project"
					value={githubAccountIdentifierToString(account)}
					options={githubAccounts.current.map((account) => ({
						label: account.info.username,
						value: githubAccountIdentifierToString(account)
					}))}
					onselect={(value) => {
						const account = stringToGitHubAccountIdentifier(value);
						if (!account) return;
						projectsService.updatePreferredForgeUser(projectId, {
							provider: 'github',
							details: account
						});
					}}
					disabled={githubAccounts.current.length <= 1}
					wide
				>
					{#snippet itemSnippet({ item, highlighted })}
						{@const itemAccount = item.value && stringToGitHubAccountIdentifier(item.value)}
						<SelectItem
							selected={item.value === githubAccountIdentifierToString(account)}
							{highlighted}
						>
							{item.label}

							{#if itemAccount}
								<GitHubAccountBadge account={itemAccount} class="m-l-4" />
							{/if}
						</SelectItem>
					{/snippet}
				</Select>
			{/if}
		</CardGroup.Item>
	{/if}
</CardGroup>

{#snippet openSettingsButton()}
	<div class="forge-form__open-settings-container">
		<Button onclick={() => openGeneralSettings('integrations')} style="pop"
			>Go to General Settings</Button
		>
	</div>
{/snippet}

<style lang="scss">
	.forge-form__open-settings-container {
		display: flex;
		justify-content: center;
	}
</style>
