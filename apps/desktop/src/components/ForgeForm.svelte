<script lang="ts">
	import { DEFAULT_FORGE_FACTORY } from '$lib/forge/forgeFactory.svelte';
	import { GITLAB_STATE } from '$lib/forge/gitlab/gitlabState.svelte';
	import { PROJECTS_SERVICE } from '$lib/project/projectsService';
	import { inject } from '@gitbutler/shared/context';
	import { Link, SectionCard, Select, SelectItem, Spacer, Textbox } from '@gitbutler/ui';

	import type { ForgeName } from '$lib/forge/interface/forge';
	import type { Project } from '$lib/project/project';

	const { projectId }: { projectId: string } = $props();

	const forge = inject(DEFAULT_FORGE_FACTORY);
	const gitLabState = inject(GITLAB_STATE);
	const determinedForgeType = forge.determinedForgeType;
	const token = gitLabState.token;
	const forkProjectId = gitLabState.forkProjectId;
	const upstreamProjectId = gitLabState.upstreamProjectId;
	const instanceUrl = gitLabState.instanceUrl;

	const projectsService = inject(PROJECTS_SERVICE);
	const projectResult = $derived(projectsService.getProject(projectId));
	const project = $derived(projectResult.current.data);

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
	<SectionCard roundedBottom={forge.current.name !== 'gitlab'}>
		{#snippet title()}
			Forge override
		{/snippet}

		{#snippet caption()}
			{#if $determinedForgeType === 'default'}
				We couldn't detect which Forge you're using.
				<br />
				To enable Forge integration, please select your Forge from the dropdown below.
				<br />
				<span class="text-bold">Note:</span> Currently, only GitHub and GitLab support pull request creation.
			{:else}
				We’ve detected that you’re using <span class="text-bold"
					>{$determinedForgeType.toUpperCase()}</span
				>.
				<br />
				At the moment, it’s not possible to manually override the detected forge type.
			{/if}
		{/snippet}

		{#if $determinedForgeType === 'default'}
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
		{:else}{/if}
	</SectionCard>

	{#if forge.current.name === 'gitlab'}
		<SectionCard roundedTop={false} roundedBottom={false}>
			{#snippet title()}
				Configure GitLab integration
			{/snippet}

			{#snippet caption()}
				Learn how find your GitLab Personal Token and Project ID in our <Link
					href="https://docs.gitbutler.com/features/forge-integration/gitlab-integration">docs</Link
				>
				<br />
				The Fork Project ID is where your branches will be pushed, and the Upstream Project ID is where
				you want merge requests to be created.
				<br />
			{/snippet}

			<Textbox label="Personal token" value={$token} oninput={(value) => ($token = value)} />
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
		</SectionCard>

		<SectionCard roundedTop={false}>
			{#snippet caption()}
				If you use a custom GitLab instance (not gitlab.com), you will need to add it as a custom
				CSP entry so that GitButler trusts connecting to that host. Read more in the <Link
					href="https://docs.gitbutler.com/troubleshooting/custom-csp">docs</Link
				>
			{/snippet}
		</SectionCard>
	{/if}
</div>
<Spacer />
