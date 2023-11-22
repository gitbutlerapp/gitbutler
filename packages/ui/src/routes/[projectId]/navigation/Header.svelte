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
	<div class="header__sync text-base-11 font-semibold">
		<IconButton
			class="items-center justify-center align-top "
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
			<div class:animate-spin={fetching}>
				<IconRefresh class="h-4 w-4" />
			</div>
		</IconButton>
		<Tooltip label="Last fetch from upstream">
			{#if $base$?.fetchedAt}
				<TimeAgo date={$base$.fetchedAt} />
			{/if}
		</Tooltip>
	</div>
</div>

<style lang="postcss">
	.header {
		display: flex;
		flex-shrink: 0;
		align-items: center;
		justify-content: right;
		color: var(--clr-theme-scale-ntrl-50);
	}
	.header__sync {
		display: flex;
		align-items: center;
		gap: var(--space-4);
		padding-top: var(--space-2);
		padding-bottom: var(--space-2);
		padding-left: var(--space-6);
		padding-right: var(--space-6);
		background: var(--clr-theme-container-pale);
		border-radius: var(--radius-m);
	}
</style>
