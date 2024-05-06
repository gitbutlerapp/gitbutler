<script lang="ts">
	import { Project, ProjectService } from '$lib/backend/projects';
	import SectionCard from '$lib/components/SectionCard.svelte';
	import Toggle from '$lib/components/Toggle.svelte';
	import Section from '$lib/components/settings/Section.svelte';
	import { getContext } from '$lib/utils/context';
	import { onMount } from 'svelte';

	const projectService = getContext(ProjectService);
	const project = getContext(Project);

	let initialized = false;

	// Setting via a function to ensure we don't react to initialized
	function setSnapshotsEnabled(value: boolean) {
		if (!initialized) return;
		projectService.setSnapshotsEnabled(project, value);
	}

	let snapshotsEnabled = false;
	$: setSnapshotsEnabled(snapshotsEnabled);

	onMount(async () => {
		snapshotsEnabled = await projectService.snapshotsEnabled(project);

		initialized = true;
	});
</script>

<Section spacer>
	<svelte:fragment slot="title">Experimental settings</svelte:fragment>
	<svelte:fragment slot="description">
		This sections contains a list of feature flags for features that are still in development or in
		an experimental stage.
	</svelte:fragment>

	<SectionCard labelFor="snapshotsEnabled" orientation="row">
		<svelte:fragment slot="title">Snapshots</svelte:fragment>
		<svelte:fragment slot="caption">
			When enabled GitButler will take a snapshot after every important action. You can revert a
			snapshot by visiting the snapshot menu (Cmd+Shift+H) and clicking "Revert".
		</svelte:fragment>
		<svelte:fragment slot="actions">
			<Toggle
				id="snapshotsEnabled"
				checked={snapshotsEnabled}
				on:change={() => (snapshotsEnabled = !snapshotsEnabled)}
			/>
		</svelte:fragment>
	</SectionCard>
</Section>
