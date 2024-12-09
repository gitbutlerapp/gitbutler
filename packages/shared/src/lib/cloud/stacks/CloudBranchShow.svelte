<script lang="ts">
	import { CloudBranchesService } from '$lib/cloud/stacks/service';
	import { getContext } from '$lib/context';
	import { getRoutesService } from '$lib/sharedRoutes';
	import Button from '@gitbutler/ui/Button.svelte';
	import dayjs from 'dayjs';
	import relativeTime from 'dayjs/plugin/relativeTime';
	import { goto } from '$app/navigation';

	dayjs.extend(relativeTime);

	interface Props {
		cloudBranchId: string;
	}

	const { cloudBranchId }: Props = $props();

	const cloudBranchesService = getContext(CloudBranchesService);
	const repositoryId = cloudBranchesService.repositoryId;
	const optionalBranch = $derived(cloudBranchesService.branchForId(cloudBranchId));

	const routesService = getRoutesService();
</script>

{#if $optionalBranch.state === 'uninitialized'}
	<p>Loading...</p>
{:else if $optionalBranch.state === 'not-found'}
	<p>Error: Stack not found</p>
{:else if $optionalBranch.state === 'found'}
	{@const cloudBranch = $optionalBranch.value}

	<h1 class="text-head-24 padding-bottom">{cloudBranch.title}</h1>
	<div class="two-by-two padding-bottom">
		<div class="card">
			<div class="card__content">
				<p>Version: {cloudBranch.version}</p>
				<p>Status: {cloudBranch.status}</p>
				<p>Created at: {dayjs(cloudBranch.createdAt).fromNow()}</p>
			</div>
		</div>

		<div class="card">
			<p class="card__header text-15 text-bold">Contributors:</p>
			<div class="card__content">
				<ul>
					{#each cloudBranch.contributors as contributor}
						<li>{contributor}</li>
					{/each}
				</ul>
			</div>
		</div>
	</div>

	<h2 class="text-head-20 padding-bottom">Patches: ({cloudBranch.patches.length})</h2>

	<div class="card">
		{#each cloudBranch.patches as patch}
			<div class="line-item">
				<div>
					<p class="text-15 text-bold">{patch.title || 'Unnamed'}</p>
					<p>Commit: {patch.commitSha.slice(0, 7)} - Change: {patch.changeId.slice(0, 7)}</p>
					<p>Version: {patch.version}</p>
				</div>
				<Button
					style="pop"
					kind="solid"
					onclick={() => {
						if ($repositoryId) {
							goto(routesService.patch($repositoryId, cloudBranchId, patch.changeId));
						}
					}}>Visit</Button
				>
			</div>
		{/each}
	</div>
{/if}

<style lang="postcss">
	.padding-bottom {
		margin-bottom: 16px;
	}

	.two-by-two {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: 8px;
	}

	.line-item {
		padding: 8px;

		display: flex;
		justify-content: space-between;

		&:not(:last-child) {
			border-bottom: 1px solid var(--clr-border-1);
		}
	}
</style>
