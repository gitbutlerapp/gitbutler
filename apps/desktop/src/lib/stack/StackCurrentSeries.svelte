<script lang="ts">
	import { getForge } from '$lib/forge/interface/forge';
	import { createForgeChecksMonitorStore } from '$lib/forge/interface/forgeChecksMonitor';
	import { getForgeListingService } from '$lib/forge/interface/forgeListingService';
	import { createForgePrMonitorStore } from '$lib/forge/interface/forgePrMonitor';
	import { getForgePrService } from '$lib/forge/interface/forgePrService';
	import type { PatchSeries } from '$lib/vbranches/types';
	import type { Snippet } from 'svelte';

	interface Props {
		currentSeries: PatchSeries;
		children: Snippet;
	}

	const { currentSeries, children }: Props = $props();
	const prService = getForgePrService();
	const forge = getForge();

	// Pretty cumbersome way of getting the PR number, would be great if we can
	// make it more concise somehow.
	const hostedListingServiceStore = getForgeListingService();
	const prStore = $derived($hostedListingServiceStore?.prs);
	const prs = $derived(prStore ? $prStore : undefined);

	const listedPr = $derived(prs?.find((pr) => pr.sourceBranch === currentSeries.name));
	const prId = $derived(listedPr?.id);

	const prMonitor = $derived(prId ? $prService?.prMonitor(prId) : undefined);
	const pr = $derived(prMonitor?.pr);

	const forgePrMonitorStore = createForgePrMonitorStore(undefined);
	$effect(() => forgePrMonitorStore.set(prMonitor));

	const checksMonitor = $derived(
		$pr?.sourceBranch ? $forge?.checksMonitor($pr.sourceBranch) : undefined
	);
	const forgeChecksMonitorStore = createForgeChecksMonitorStore(undefined);
	$effect(() => forgeChecksMonitorStore.set(checksMonitor));
</script>

<div class="branch-group">
	{@render children()}
</div>

<style>
	.branch-group {
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
		background: var(--clr-bg-1);

		&:last-child {
			margin-bottom: 12px;
		}
	}
</style>
