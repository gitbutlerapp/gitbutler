<script lang="ts">
	import CommitSigningForm from './CommitSigningForm.svelte';
	import KeysForm from '../KeysForm.svelte';
	import Section from '../Section.svelte';
	import { Project, ProjectsService } from '$lib/backend/projects';
	import SectionCard from '$lib/components/SectionCard.svelte';
	import { platformName } from '$lib/platform/platform';
	import Spacer from '$lib/shared/Spacer.svelte';
	import { getContext } from '@gitbutler/shared/context';
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
	{#if $platformName !== 'win32'}
		<Spacer />
		<KeysForm showProjectName={false} />
	{/if}

	<Spacer />
	<SectionCard orientation="row" labelFor="allowForcePush">
		<svelte:fragment slot="title">Allow force pushing</svelte:fragment>
		<svelte:fragment slot="caption">
			Force pushing allows GitButler to override branches even if they were pushed to remote.
			GitButler will never force push to the target branch.
		</svelte:fragment>
		<svelte:fragment slot="actions">
			<Toggle id="allowForcePush" checked={allowForcePushing} onclick={handleAllowForcePushClick} />
		</svelte:fragment>
	</SectionCard>
</Section>
