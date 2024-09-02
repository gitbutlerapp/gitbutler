<script lang="ts">
	import { ProjectService } from '$lib/backend/projects';
	import { getContext } from '$lib/utils/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import type { FailedToOpenRepoInexistentError } from '$lib/utils/errors';

	const projectService = getContext(ProjectService);

	interface Props {
		error: FailedToOpenRepoInexistentError;
	}
	const { error }: Props = $props();

	let deleteSucceeded: boolean | undefined = $state(undefined);

	async function stopTracking() {
		deleteSucceeded = await projectService.deleteProjectByPath(error.path);
	}
</script>

<div class="container">
	{#if deleteSucceeded === undefined}
		<p class="text-12 text-body text-bold">
			Do you want to stop tracking the repo under this path?:
		</p>
		<p class="text-12 text-body repo-path">{error.path}</p>
		<div class="button-container">
			<Button style="error" onclick={stopTracking}>Remove</Button>
		</div>
	{/if}

  {#if deleteSucceeded === true}
    <p class="text-12 text-body text-bold">
      Repo removed successfully
    </p>
  {/if}

  {#if deleteSucceeded === false}
    <p class="text-12 text-body text-bold">
      Failed to remove repo
    </p>
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
