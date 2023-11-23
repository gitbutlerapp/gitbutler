<script lang="ts">
	import IconButton from '$lib/components/IconButton.svelte';
	import TimeAgo from '$lib/components/TimeAgo.svelte';
	import Tooltip from '$lib/components/Tooltip.svelte';
	import type { PrService } from '$lib/github/pullrequest';
	import IconRefresh from '$lib/icons/IconRefresh.svelte';
	import type { BranchController } from '$lib/vbranches/branchController';
	import type { BaseBranchService } from '$lib/vbranches/branchStoresCache';

	export let branchController: BranchController;
	export let prService: PrService;
	export let baseBranchService: BaseBranchService;

	$: base$ = baseBranchService.base$;

	let fetching = false;
</script>

<div data-tauri-drag-region class="header">
	<button
		class="sync-btn"
		on:click={async (e) => {
			e.preventDefault();
			e.stopPropagation();
			fetching = true;
			await branchController.fetchFromTarget().finally(() => {
				fetching = false;
				prService.reload();
			});
		}}
	>
		<Tooltip label="Last fetch from upstream">
			{#if $base$?.fetchedAt}
				<span class="text-base-11 text-semibold sync-btn__label">
					{#if fetching}
						fetching...
					{:else}
						<TimeAgo date={$base$?.fetchedAt} />
					{/if}
				</span>
			{/if}
		</Tooltip>
	</button>
</div>

<style lang="postcss">
	.header {
		display: flex;
		flex-shrink: 0;
		align-items: center;
		justify-content: right;
	}

	.sync-btn {
		display: flex;
		align-items: center;
		gap: var(--space-4);
		padding-top: var(--space-2);
		padding-bottom: var(--space-2);
		padding-left: var(--space-6);
		padding-right: var(--space-6);
		background: var(--clr-theme-container-pale);
		border-radius: var(--radius-m);
		transition: background var(--transition-fast);

		&:hover {
			background: var(--clr-theme-container-sub);
		}
	}

	.sync-btn__label {
		white-space: nowrap;
		color: var(--clr-theme-scale-ntrl-50);
	}
</style>
