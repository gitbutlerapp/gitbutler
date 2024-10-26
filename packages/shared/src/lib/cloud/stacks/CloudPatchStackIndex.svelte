<script lang="ts">
	import { CloudPatchStacksService } from '$lib/cloud/stacks/service';
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
	 * - CloudPatchStacksService
	 * - RoutesService
	 */

	const patchStacksService = getContext(CloudPatchStacksService);
	const routesService = getRoutesService();
	const patchStacks = $derived(patchStacksService.patchStacks);
</script>

{#if $patchStacks}
	<div class="card">
		{#each $patchStacks as patchStack}
			<div class="line-item">
				<div>
					<p class="text-15 text-bold">{patchStack.title}</p>
					<p>Version: v{patchStack.version}</p>
					<p>Status: {patchStack.status}</p>
					<p>Created: {dayjs(patchStack.createdAt).fromNow()}</p>
				</div>
				<Button
					style="pop"
					kind="solid"
					onclick={() => {
						const repositoryId = get(patchStacksService.repositoryId);
						if (repositoryId) {
							goto(routesService.patchStack(repositoryId, patchStack.uuid));
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
