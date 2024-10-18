<script lang="ts">
	import { CloudRepositoriesService } from '@gitbutler/shared/cloud/repositories/service';
	import { getContext } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import { goto } from '$app/navigation';

	const cloudRepositoriesService = getContext(CloudRepositoriesService);
	const repositories = $derived(cloudRepositoriesService.repositories);

	$inspect($repositories);
</script>

<h2>Your projects:</h2>

{#if !$repositories}
	<p>Loading...</p>
{:else if $repositories.length === 0}
	<p>
		You've not got any projects added yet. Enable project syncing in GitButler in order to see
		projects.
	</p>
{:else}
	<div class="card">
		{#each $repositories as repository}
			<div class="line-item">
				<div>
					<h5 class="text-head-22">{repository.name}</h5>
					<p>{repository.name}</p>
				</div>
				<Button
					style="pop"
					kind="solid"
					onclick={() => {
						goto(`/repositories/${repository.repositoryId}`);
					}}>Visit</Button
				>
			</div>
		{/each}
	</div>
{/if}

<style>
	.line-item {
		padding: 8px;

		display: flex;
		justify-content: space-between;

		&:not(:last-child) {
			border-bottom: 1px solid var(--clr-border-1);
		}
	}
</style>
