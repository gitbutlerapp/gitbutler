<script lang="ts">
	import { getColorFromBranchType } from '$lib/branch/stackingUtils';
	import { getForge } from '$lib/forge/interface/forge';
	import { createForgeChecksMonitorStore } from '$lib/forge/interface/forgeChecksMonitor';
	import { getForgeListingService } from '$lib/forge/interface/forgeListingService';
	import { createForgePrMonitorStore } from '$lib/forge/interface/forgePrMonitor';
	import { createForgePrServiceStore } from '$lib/forge/interface/forgePrService';
	import type { PatchSeries } from '$lib/vbranches/types';
	import type { Snippet } from 'svelte';

	interface Props {
		currentSeries: PatchSeries;
		children: Snippet;
	}

	const { currentSeries, children }: Props = $props();

	// Setup PR Store and Monitor on a per-series basis
	const forge = getForge();
	const prService = createForgePrServiceStore(undefined);
	$effect(() => prService.set($forge?.prService()));

	// Pretty cumbersome way of getting the PR number, would be great if we can
	// make it more concise somehow.
	const hostedListingServiceStore = getForgeListingService();
	const prStore = $derived($hostedListingServiceStore?.prs);
	const prs = $derived(prStore ? $prStore : undefined);

	const listedPr = $derived(prs?.find((pr) => pr.sourceBranch === currentSeries.name));
	const prNumber = $derived(listedPr?.number);

	const prMonitor = $derived(prNumber ? $prService?.prMonitor(prNumber) : undefined);
	const pr = $derived(prMonitor?.pr);

	const forgePrMonitorStore = createForgePrMonitorStore(undefined);
	$effect(() => forgePrMonitorStore.set(prMonitor));

	const checksMonitor = $derived(
		$pr?.sourceBranch ? $forge?.checksMonitor($pr.sourceBranch) : undefined
	);
	const forgeChecksMonitorStore = createForgeChecksMonitorStore(undefined);
	$effect(() => forgeChecksMonitorStore.set(checksMonitor));

	const seriesType = currentSeries.patches[0] ? currentSeries.patches[0].status : 'local';
</script>

<div
	class="branch-group"
	data-series-name={currentSeries.name}
	style:--highlight-color={getColorFromBranchType(seriesType)}
>
	{@render children()}
</div>

<style>
	.branch-group {
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
		background: var(--clr-bg-1);
		scroll-margin-top: 120px;

		&:last-child {
			margin-bottom: 12px;
		}
	}
</style>
