<script lang="ts">
	import { syncToCloud } from '$lib/backend/cloud';
	import Icon from '$lib/icons/Icon.svelte';
	import TimeAgo from '$lib/components/TimeAgo.svelte';
	import Tooltip from '$lib/components/Tooltip.svelte';
	import type { PrService } from '$lib/github/pullrequest';
	import type { BranchController } from '$lib/vbranches/branchController';
	import type { BaseBranchService } from '$lib/vbranches/branchStoresCache';

	export let branchController: BranchController;
	export let prService: PrService;
	export let projectId: string;
	export let baseBranchService: BaseBranchService;
	export let cloudEnabled: boolean;

	$: base$ = baseBranchService.base$;

	let fetching = false;
</script>

<button
	class="sync-btn"
	on:click={async (e) => {
		e.preventDefault();
		e.stopPropagation();
		fetching = true;
		try {
			if (cloudEnabled) syncToCloud(projectId); // don't wait for this
			await branchController.fetchFromTarget();
			await prService.reload();
		} finally {
			fetching = false;
		}
	}}
>
	{#if !fetching}
		<div class="sync-btn__icon">
			<Icon name="update-small" />
		</div>
	{/if}

	<Tooltip label="Last fetch from upstream">
		{#if $base$?.fetchedAt}
			<span class="text-base-11 text-semibold sync-btn__label">
				{#if fetching}
					<div class="sync-btn__busy-label">busy…</div>
				{:else}
					<TimeAgo date={$base$?.fetchedAt} />
				{/if}
			</span>
		{/if}
	</Tooltip>
</button>

<style lang="postcss">
	.sync-btn {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		height: var(--space-20);
		padding-left: var(--space-2);
		padding-right: var(--space-4);
		background: var(--clr-theme-container-light);
		border: 1px solid var(--clr-theme-container-outline-light);
		border-radius: var(--radius-m);
		transition:
			background var(--transition-fast),
			border var(--transition-fast);

		&:hover {
			background: var(--clr-theme-container-light);
			border: 1px solid var(--clr-theme-container-outline-pale);
		}

		&:hover .sync-btn__icon {
			fill: var(--clr-theme-scale-ntrl-40);
		}

		&:hover .sync-btn__label {
			color: var(--clr-theme-scale-ntrl-40);
		}
	}

	.sync-btn__icon {
		display: flex;
		color: var(--clr-theme-scale-ntrl-40);
		transform-origin: center;
		transform: rotate(0deg);
		transition:
			fill var(--transition-fast),
			transform var(--transition-slow);
	}

	.sync-btn__label {
		display: block;
		line-height: 1;
		white-space: nowrap;
		color: var(--clr-theme-scale-ntrl-40);
		transition: color var(--transition-fast);
	}

	.sync-btn__busy-label {
		padding-left: var(--space-4);
	}
</style>
