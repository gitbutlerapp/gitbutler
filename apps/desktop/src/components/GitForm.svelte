<script lang="ts">
	import CommitSigningForm from '$components/CommitSigningForm.svelte';
	import KeysForm from '$components/KeysForm.svelte';
	import Section from '$components/Section.svelte';
	import { platformName } from '$lib/platform/platform';
	import { Project } from '$lib/project/project';
	import { ProjectsService } from '$lib/project/projectsService';
	import { getContext } from '@gitbutler/shared/context';
	import SectionCard from '@gitbutler/ui/SectionCard.svelte';
	import Spacer from '@gitbutler/ui/Spacer.svelte';
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
</script>

<Section>
	<CommitSigningForm />
	{#if platformName !== 'windows'}
		<Spacer />
		<KeysForm showProjectName={false} />
	{/if}

	<Spacer />
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
</Section>
