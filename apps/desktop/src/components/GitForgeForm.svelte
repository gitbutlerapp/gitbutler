<script lang="ts">
	import { projectDeleteRemoteBranchAfterMerge } from '$lib/config/config';
	import { Project } from '$lib/project/project';
	import { ProjectsService } from '$lib/project/projectsService';
	import { getContext } from '@gitbutler/shared/context';
	import SectionCard from '@gitbutler/ui/SectionCard.svelte';
	import Toggle from '@gitbutler/ui/Toggle.svelte';

	const project = getContext(Project);
	const projectsService = getContext(ProjectsService);

	const allowForcePushing = $derived(project?.ok_with_force_push);

	async function setWithForcePush(value: boolean) {
		project.ok_with_force_push = value;
		await projectsService.updateProject(project);
	}

	async function handleAllowForcePushClick(event: MouseEvent) {
		await setWithForcePush((event.target as HTMLInputElement)?.checked);
	}

	const deleteRemoteBranchEnabled = projectDeleteRemoteBranchAfterMerge(project.id);
</script>

<SectionCard orientation="row" labelFor="allowForcePush">
	{#snippet title()}
		Allow force pushing
	{/snippet}
	{#snippet caption()}
		Force pushing allows GitButler to override branches even if they were pushed to remote.
		GitButler will never force push to the target branch.
	{/snippet}
	{#snippet actions()}
		<Toggle id="allowForcePush" checked={allowForcePushing} onclick={handleAllowForcePushClick} />
	{/snippet}
</SectionCard>

<SectionCard orientation="row" labelFor="deleteRemoteBranchAfterMerge">
	{#snippet title()}
		Delete Remote Branch After Merge
	{/snippet}
	{#snippet caption()}
		If you do not have "Automatically delete head branches" enabled in your repository, GitButler
		can automatically delete the remote branch from your remote for you after successfully merging.
	{/snippet}
	{#snippet actions()}
		<Toggle id="deleteRemoteBranchAfterMerge" bind:checked={$deleteRemoteBranchEnabled} />
	{/snippet}
</SectionCard>
