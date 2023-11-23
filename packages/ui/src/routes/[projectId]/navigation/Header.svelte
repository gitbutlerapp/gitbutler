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
		{#if !fetching}
			<svg viewBox="0 0 12 12" class="sync-btn__icon" xmlns="http://www.w3.org/2000/svg">
				<path
					fill-rule="evenodd"
					clip-rule="evenodd"
					d="M6 12C9.31371 12 12 9.31371 12 6C12 2.68629 9.31371 0 6 0C2.68629 0 0 2.68629 0 6C0 9.31371 2.68629 12 6 12ZM6 3.59999C4.67452 3.59999 3.6 4.67451 3.6 5.99999C3.6 7.32548 4.67452 8.39999 6 8.39999C6.71074 8.39999 7.34871 8.0918 7.78903 7.59985L8.68319 8.40014C8.02486 9.13568 7.06626 9.59999 6 9.59999C4.01178 9.59999 2.4 7.98822 2.4 5.99999C2.4 4.01177 4.01178 2.39999 6 2.39999C7.74209 2.39999 9.19517 3.63741 9.52824 5.28125H10.6719L8.88281 7.53906L7.09375 5.28125H8.29052C7.98509 4.3069 7.0751 3.59999 6 3.59999Z"
				/>
			</svg>
		{/if}

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
		height: 20px;
		gap: var(--space-6);
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

		&:hover .sync-btn__icon {
			fill: var(--clr-theme-scale-ntrl-40);
			transform: rotate(180deg);
		}

		&:hover .sync-btn__label {
			color: var(--clr-theme-scale-ntrl-40);
		}
	}

	.sync-btn__icon {
		fill: var(--clr-theme-scale-ntrl-50);
		width: var(--space-12);
		height: var(--space-12);
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
		color: var(--clr-theme-scale-ntrl-50);
		transition: color var(--transition-fast);
	}
</style>
