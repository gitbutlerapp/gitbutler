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

	const forge = getContext(DefaultForgeFactory);
	const gitLabState = getContext(GitLabState);
	const determinedForgeType = forge.determinedForgeType;
	const token = gitLabState.token;
	const forkProjectId = gitLabState.forkProjectId;
	const upstreamProjectId = gitLabState.upstreamProjectId;
	const instanceUrl = gitLabState.instanceUrl;

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

<Spacer />
<SectionCard>
	<h3 class="text-bold text-15">Forge override</h3>
	{#if $determinedForgeType === 'default'}
		<p class="text-13">
			We were unable to determine what forge you were using. In order to make use of a forge
			integration, select your forge of it from the dropdown below.
		</p>
		<p class="text-13">
			Please note that only the GitHub and GitLab support PR creation at the moment.
		</p>
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
	{:else}
		<p class="text-13">
			We have determined that you are currently using <code>{$determinedForgeType}</code>. We
			currently do not support overriding an automatically determined forge type.
		</p>
	{/if}
</SectionCard>

{#if forge.current.name === 'gitlab'}
	<Spacer />
	<SectionCard>
		<h3 class="text-bold text-15">Configure GitLab Integration</h3>
		<p class="text-13">
			Learn how find your GitLab Personal Token and Project ID in our <Link
				href="https://docs.gitbutler.com/features/gitlab-integration">documentation</Link
			>
		</p>
		<p class="text-13">
			The Fork Project ID should be the project that your branches get pushed to, and the Upstream
			Project ID should be project where you want the Merge Requests to be created.
		</p>

		<Textbox label="Personal Token" value={$token} oninput={(value) => ($token = value)} />
		<Textbox
			label="Your Fork's Project ID"
			value={$forkProjectId}
			oninput={(value) => ($forkProjectId = value)}
		/>
		<Textbox
			label="Upstream Project ID"
			value={$upstreamProjectId}
			oninput={(value) => ($upstreamProjectId = value)}
		/>
		<Textbox
			label="Instance URL"
			value={$instanceUrl}
			oninput={(value) => ($instanceUrl = value)}
		/>
		<p class="text-13">
			If you use a custom GitLab instance (not gitlab.com), you will need to add it as a custom CSP
			entry so that GitButler trusts connecting to that host. Read more in the <Link
				href="https://docs.gitbutler.com/troubleshooting/custom-csp">docs</Link
			>
		</p>
	</SectionCard>
{/if}
