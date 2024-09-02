<script lang="ts">
	import RemoveProjectButton from '../RemoveProjectButton.svelte';
	import { ProjectService } from '$lib/backend/projects';
	import { getContext } from '$lib/utils/context';

	const projectService = getContext(ProjectService);

	let isDeleting = $state(false);
	let deleteSucceeded: boolean | undefined = $state(undefined);

	async function stopTracking() {
		isDeleting = true;
		deleteProject: {
			const id = projectService.getLastOpenedProject();
			if (id === undefined) {
				deleteSucceeded = false;
				break deleteProject;
			}
			await projectService.deleteProject(id);
		}
		isDeleting = false;
	}
</script>

<div class="container">
	{#if deleteSucceeded === undefined}
		<p class="text-12 text-body text-bold">Do you want to:</p>
		<div class="button-container">
			<RemoveProjectButton noModal {isDeleting} onDeleteClicked={stopTracking} />
		</div>
	{/if}

	{#if deleteSucceeded === true}
		<p class="text-12 text-body text-bold">Repo removed successfully</p>
	{/if}

	{#if deleteSucceeded === false}
		<p class="text-12 text-body text-bold">Failed to remove repo</p>
	{/if}
</div>

<style lang="postcss">
	.container {
		display: flex;
		flex-direction: column;
		gap: 12px;
	}

	.button-container {
		display: flex;
		justify-content: center;
	}
</style>
