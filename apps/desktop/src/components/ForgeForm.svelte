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
	import { Button, CardGroup, Link, Select, SelectItem } from '@gitbutler/ui';

	import type { ForgeName } from '$lib/forge/interface/forge';
	import type { Project } from '$lib/project/project';
	import type { ButGitHubToken, ButGitLabToken } from '@gitbutler/core/api';

	type AccountIdentifier =
		| ButGitHubToken.GithubAccountIdentifier
		| ButGitLabToken.GitlabAccountIdentifier;

	function getAccountUsername(account: AccountIdentifier): string {
		return account.info.username;
	}

	const FORGE_OPTIONS: { label: string; value: ForgeName }[] = [
		{ label: 'None', value: 'default' },
		{ label: 'GitHub', value: 'github' },
		{ label: 'GitLab', value: 'gitlab' },
		{ label: 'Azure', value: 'azure' },
		{ label: 'BitBucket', value: 'bitbucket' }
	];

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

	{#if forge.current.name === 'gitlab'}
		{@render forgeAccountConfig({
			providerName: 'gitlab',
			displayName: 'GitLab',
			accounts: gitlabAccounts.current,
			preferredAccount: preferredGitLabAccount.current,
			accountToString: gitlabAccountIdentifierToString,
			stringToAccount: stringToGitLabAccountIdentifier,
			AccountBadge: GitLabAccountBadge,
			docsUrl: 'https://docs.gitbutler.com/features/forge-integration/gitlab-integration',
			requestType: 'merge request'
		})}
	{/if}

	{#if forge.current.name === 'github'}
		{@render forgeAccountConfig({
			providerName: 'github',
			displayName: 'GitHub',
			accounts: githubAccounts.current,
			preferredAccount: preferredGitHubAccount.current,
			accountToString: githubAccountIdentifierToString,
			stringToAccount: stringToGitHubAccountIdentifier,
			AccountBadge: GitHubAccountBadge,
			docsUrl: 'https://docs.gitbutler.com/features/forge-integration/github-integration',
			requestType: 'pull request'
		})}
	{/if}
</CardGroup>

{#snippet forgeAccountConfig({
	providerName,
	displayName,
	accounts,
	preferredAccount,
	accountToString,
	stringToAccount,
	AccountBadge,
	docsUrl,
	requestType
}: {
	providerName: 'github' | 'gitlab';
	displayName: string;
	accounts: AccountIdentifier[];
	preferredAccount: AccountIdentifier | undefined;
	accountToString: (account: any) => string;
	stringToAccount: (value: string) => any;
	AccountBadge: typeof GitHubAccountBadge | typeof GitLabAccountBadge;
	docsUrl: string;
	requestType: string;
})}
	<CardGroup.Item>
		{#snippet title()}
			{#if accounts.length === 0 || !preferredAccount}
				No {displayName} accounts found
			{:else}
				Configure {displayName} integration
			{/if}
		{/snippet}

		{#snippet caption()}
			Enable {requestType} creation. Read more in the <Link href={docsUrl}>docs</Link>
		{/snippet}

		{#if accounts.length === 0 || !preferredAccount}
			{@render openSettingsButton()}
		{:else}
			{@const account = preferredAccount}
			<Select
				label="{displayName} account for this project"
				value={accountToString(account)}
				options={accounts.map((account) => ({
					label: getAccountUsername(account),
					value: accountToString(account)
				}))}
				onselect={(value) => {
					const account = stringToAccount(value);
					if (!account) return;
					projectsService.updatePreferredForgeUser(projectId, {
						provider: providerName,
						details: account
					});
				}}
				disabled={accounts.length <= 1}
				wide
			>
				{#snippet itemSnippet({ item, highlighted })}
					{@const itemAccount = item.value && stringToAccount(item.value)}
					<SelectItem selected={item.value === accountToString(account)} {highlighted}>
						{item.label}

						{#if itemAccount}
							<AccountBadge account={itemAccount} class="m-l-4" />
						{/if}
					</SelectItem>
				{/snippet}
			</Select>
		{/if}
	</CardGroup.Item>
{/snippet}

{#snippet openSettingsButton()}
	<div class="flex">
		<Button onclick={() => openGeneralSettings('integrations')} style="pop" icon="link"
			>Set up in General Settings</Button
		>
	</div>
{/snippet}
