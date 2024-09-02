<script lang="ts">
	import RemoveProjectButton from '../RemoveProjectButton.svelte';
	import { ProjectService } from '$lib/backend/projects';
	import { getContext } from '$lib/utils/context';
	import type { FailedToOpenRepoInexistentError } from '$lib/utils/errors';

	const projectService = getContext(ProjectService);

	interface Props {
		error: FailedToOpenRepoInexistentError;
	}
	const { error }: Props = $props();

	let isDeleting = $state(false);
	let deleteSucceeded: boolean | undefined = $state(undefined);

	async function stopTracking() {
		isDeleting = true;
		deleteSucceeded = await projectService.deleteProjectByPath(error.path);
		isDeleting = false;
	}
</script>

<div class="container">
	{#if deleteSucceeded === undefined}
		<p class="text-12 text-body text-bold">
			Do you want to stop tracking the repo under this path?:
		</p>
		<p class="text-12 text-body repo-path">{error.path}</p>
		<div class="button-container">
			<RemoveProjectButton
				noModal
				projectTitle={error.path}
				{isDeleting}
				onDeleteClicked={stopTracking}
			/>
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

	.repo-path {
		text-align: center;
	}

	.button-container {
		display: flex;
		justify-content: center;
	}
</style>
