<script lang="ts">
	import { DefaultForgeFactory } from '$lib/forge/forgeFactory.svelte';
	import { GitLabState } from '$lib/forge/gitlab/gitlabState.svelte';
	import { ProjectService } from '$lib/project/projectService';
	import { ProjectsService } from '$lib/project/projectsService';
	import { getContext } from '@gitbutler/shared/context';
	import SectionCard from '@gitbutler/ui/SectionCard.svelte';
	import Spacer from '@gitbutler/ui/Spacer.svelte';
	import Textbox from '@gitbutler/ui/Textbox.svelte';
	import Link from '@gitbutler/ui/link/Link.svelte';
	import Select from '@gitbutler/ui/select/Select.svelte';
	import SelectItem from '@gitbutler/ui/select/SelectItem.svelte';
	import type { ForgeName } from '$lib/forge/interface/forge';
	import type { Project } from '$lib/project/project';
	import { GiteaState } from '$lib/forge/gitea/giteaState.svelte';

	const forge = getContext(DefaultForgeFactory);
	const gitLabState = getContext(GitLabState);
	const giteaState = getContext(GiteaState);

	const determinedForgeType = forge.determinedForgeType;
	const gitLabToken = gitLabState.token;
	const gitLabForkProjectId = gitLabState.forkProjectId;
	const gitLabUpstreamProjectId = gitLabState.upstreamProjectId;
	const gitLabInstanceUrl = gitLabState.instanceUrl;

	const giteaToken = giteaState.token;
	const giteaForkProjectId = giteaState.forkProjectId;
	const giteaUpstreamProjectId = giteaState.upstreamProjectId;
	const giteaInstanceUrl = giteaState.instanceUrl;

	const projectsService = getContext(ProjectsService);
	const projectService = getContext(ProjectService);
	const project = projectService.project;

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
		},
		{
			label: 'Gitea',
			value: 'gitea'
		}
	];
	let selectedOption = $derived($project?.forge_override || 'default');

	function handleSelectionChange(selectedOption: ForgeName) {
		if (!$project) return;

		const mutableProject: Project & { unset_forge_override?: boolean } = structuredClone($project);

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
					href="https://docs.gitbutler.com/features/gitlab-integration">docs</Link
				>
				<br />
				The Fork Project ID is where your branches will be pushed, and the Upstream Project ID is where
				you want merge requests to be created.
				<br />
			{/snippet}

			<Textbox
				label="Personal token"
				value={$gitLabToken}
				oninput={(value) => ($gitLabToken = value)}
			/>
			<Textbox
				label="Your fork's project ID"
				value={$gitLabForkProjectId}
				oninput={(value) => ($gitLabForkProjectId = value)}
			/>
			<Textbox
				label="Upstream project ID"
				value={$gitLabUpstreamProjectId}
				oninput={(value) => ($gitLabUpstreamProjectId = value)}
			/>
			<Textbox
				label="Instance URL"
				value={$gitLabInstanceUrl}
				oninput={(value) => ($gitLabInstanceUrl = value)}
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

	{#if forge.current.name === 'gitea'}
		<SectionCard roundedTop={false} roundedBottom={false}>
			{#snippet title()}
				Configure Gitea integration
			{/snippet}

			{#snippet caption()}
				Learn how find your Gitea Personal Token and Project ID in our <Link
					href="https://docs.gitbutler.com/features/gitlab-integration">docs</Link
				>
				<br />
				The Fork Project ID is where your branches will be pushed, and the Upstream Project ID is where
				you want merge requests to be created.
				<br />
			{/snippet}

			<Textbox
				label="Personal token"
				value={$giteaToken}
				oninput={(value) => ($giteaToken = value)}
			/>
			<Textbox
				label="Your fork's project ID"
				value={$giteaForkProjectId}
				oninput={(value) => {
					// $giteaForkProjectId = value
				}}
				helperText="Must be a valid Gitea ID including owner/repo"
			/>
			<Textbox
				label="Upstream project ID"
				value={$giteaUpstreamProjectId}
				oninput={(value) => ($giteaUpstreamProjectId = value)}
				helperText="Must be a valid Gitea ID including owner/repo"
			/>
			<Textbox
				label="Instance URL"
				value={$giteaInstanceUrl}
				oninput={(value) => ($giteaInstanceUrl = value)}
			/>
		</SectionCard>

		<SectionCard roundedTop={false}>
			{#snippet caption()}
				If you use a custom Gitea instance (not gitea.com), you will need to add it as a custom CSP
				entry so that GitButler trusts connecting to that host. Read more in the <Link
					href="https://docs.gitbutler.com/troubleshooting/custom-csp">docs</Link
				>
			{/snippet}
		</SectionCard>
	{/if}
</div>
<Spacer />
