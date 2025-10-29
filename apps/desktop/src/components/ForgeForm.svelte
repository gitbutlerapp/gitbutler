<script lang="ts">
	import { DEFAULT_FORGE_FACTORY } from '$lib/forge/forgeFactory.svelte';
	import {
		githubAccountIdentifierToString,
		stringToGitHubAccountIdentifier
	} from '$lib/forge/github/githubUserService.svelte';
	import { usePreferredGitHubUsername } from '$lib/forge/github/hooks.svelte';
	import { GITLAB_STATE } from '$lib/forge/gitlab/gitlabState.svelte';
	import { PROJECTS_SERVICE } from '$lib/project/projectsService';
	import { inject } from '@gitbutler/core/context';
	import { reactive } from '@gitbutler/shared/reactiveUtils.svelte';
	import { Link, SectionCard, Select, SelectItem, Spacer, Textbox } from '@gitbutler/ui';

	import type { ForgeName } from '$lib/forge/interface/forge';
	import type { Project } from '$lib/project/project';

	const { projectId }: { projectId: string } = $props();

	const forge = inject(DEFAULT_FORGE_FACTORY);
	const gitLabState = inject(GITLAB_STATE);
	const { preferredGitHubAccount, githubAccounts } = usePreferredGitHubUsername(
		reactive(() => projectId)
	);

	const token = gitLabState.token;
	const forkProjectId = gitLabState.forkProjectId;
	const upstreamProjectId = gitLabState.upstreamProjectId;
	const instanceUrl = gitLabState.instanceUrl;

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

<div>
	<SectionCard roundedBottom={!['github', 'gitlab'].includes(forge.current.name)}>
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
	</SectionCard>

	{#if forge.current.name === 'gitlab'}
		<SectionCard roundedTop={false}>
			{#snippet title()}
				Configure GitLab integration
			{/snippet}

			{#snippet caption()}
				Learn how to find your GitLab Personal Token and Project ID in our <Link
					href="https://docs.gitbutler.com/features/forge-integration/gitlab-integration">docs</Link
				>
				<br />
				The Fork Project ID is where branches are pushed; the Upstream Project ID is where merge requests
				are created.
			{/snippet}

			<Textbox
				label="Personal token"
				type="password"
				value={$token}
				oninput={(value) => ($token = value)}
			/>
			<Textbox
				label="Your fork's project ID"
				value={$forkProjectId}
				oninput={(value) => ($forkProjectId = value)}
			/>
			<Textbox
				label="Upstream project ID"
				value={$upstreamProjectId}
				oninput={(value) => ($upstreamProjectId = value)}
			/>
			<Textbox
				label="Instance URL"
				value={$instanceUrl}
				oninput={(value) => ($instanceUrl = value)}
			/>

			<Spacer margin={5} />

			<p class="text-12 text-body clr-text-2">
				For custom GitLab instances (not gitlab.com), add them as a custom CSP entry so GitButler
				can connect. Read more in the <Link
					href="https://docs.gitbutler.com/troubleshooting/custom-csp">docs</Link
				>
			</p>
		</SectionCard>
	{/if}

	{#if forge.current.name === 'github'}
		<SectionCard roundedTop={false}>
			{#snippet title()}
				Configure GitHub integration
			{/snippet}

			{#snippet caption()}
				Enable pull request creation. Read more in the <Link
					href="https://docs.gitbutler.com/features/forge-integration/github-integration">docs</Link
				>.
			{/snippet}

			{#if githubAccounts.current.length === 0 || !preferredGitHubAccount.current}
				<!-- TODO: Link to the general settings -->
				<p>Make sure that you've logged in correctly from the General Settings</p>
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
						projectsService.updatePreferredForgeUser(projectId, account);
					}}
					disabled={githubAccounts.current.length <= 1}
					wide
				>
					{#snippet itemSnippet({ item, highlighted })}
						<SelectItem
							selected={item.value === githubAccountIdentifierToString(account)}
							{highlighted}
						>
							{item.label}
						</SelectItem>
					{/snippet}
				</Select>
			{/if}
		</SectionCard>
	{/if}
</div>
<Spacer />
