<script lang="ts">
	import { getGitHost } from '$lib/gitHost/interface/gitHost';
	import { createGitHostChecksMonitorStore } from '$lib/gitHost/interface/gitHostChecksMonitor';
	import { getGitHostListingService } from '$lib/gitHost/interface/gitHostListingService';
	import { createGitHostPrMonitorStore } from '$lib/gitHost/interface/gitHostPrMonitor';
	import { createGitHostPrServiceStore } from '$lib/gitHost/interface/gitHostPrService';
	import type { PatchSeries } from '$lib/vbranches/types';
	import type { Snippet } from 'svelte';

	interface Props {
		currentSeries: PatchSeries;
		children: Snippet;
	}

	const { currentSeries, children }: Props = $props();

	// Setup PR Store and Monitor on a per-series basis
	const gitHost = getGitHost();
	const prService = createGitHostPrServiceStore(undefined);
	$effect(() => prService.set($gitHost?.prService()));

	// Pretty cumbersome way of getting the PR number, would be great if we can
	// make it more concise somehow.
	const hostedListingServiceStore = getGitHostListingService();
	const prStore = $derived($hostedListingServiceStore?.prs);
	const prs = $derived(prStore ? $prStore : undefined);

	const listedPr = $derived(prs?.find((pr) => pr.sourceBranch === currentSeries.name));
	const prNumber = $derived(listedPr?.number);

	const prMonitor = $derived(prNumber ? $prService?.prMonitor(prNumber) : undefined);
	const pr = $derived(prMonitor?.pr);

	const gitHostPrMonitorStore = createGitHostPrMonitorStore(undefined);
	$effect(() => gitHostPrMonitorStore.set(prMonitor));

	const checksMonitor = $derived(
		$pr?.sourceBranch ? $gitHost?.checksMonitor($pr.sourceBranch) : undefined
	);
	const gitHostChecksMonitorStore = createGitHostChecksMonitorStore(undefined);
	$effect(() => gitHostChecksMonitorStore.set(checksMonitor));
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
