<script lang="ts">
	import { CloudBranchesService } from '$lib/cloud/stacks/service';
	import { getContext } from '$lib/context';
	import { getRoutesService } from '$lib/sharedRoutes';
	import Button from '@gitbutler/ui/Button.svelte';
	import dayjs from 'dayjs';
	import relativeTime from 'dayjs/plugin/relativeTime';
	import { get } from 'svelte/store';
	import { goto } from '$app/navigation';

	dayjs.extend(relativeTime);

	/**
	 * Expects the following contexts:
	 * - CloudBranchesService
	 * - RoutesService
	 */

	const cloudBranchesService = getContext(CloudBranchesService);
	const routesService = getRoutesService();
	const cloudBranches = $derived(cloudBranchesService.branches);
</script>

{#if $cloudBranches}
	<div class="card">
		{#each $cloudBranches as cloudBranch}
			<div class="line-item">
				<div>
					<p class="text-15 text-bold">{cloudBranch.title}</p>
					<p>Version: v{cloudBranch.version}</p>
					<p>Status: {cloudBranch.status}</p>
					<p>Created: {dayjs(cloudBranch.createdAt).fromNow()}</p>
				</div>
				<Button
					style="pop"
					kind="solid"
					onclick={() => {
						const repositoryId = get(cloudBranchesService.repositoryId);
						if (repositoryId) {
							goto(routesService.cloudBranch(repositoryId, cloudBranch.uuid));
						}
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
