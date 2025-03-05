<script lang="ts">
	// This page is displayed when:
	// - A pr is found
	// - And it does NOT have a cooresponding vbranch
	// - And it does NOT have a cooresponding remote
	// It may also display details about a cooresponding pr if they exist
	import FullviewLoading from '$components/FullviewLoading.svelte';
	import PullRequestPreview from '$components/PullRequestPreview.svelte';
	import { getForgeListingService } from '$lib/forge/interface/forgeListingService';
	import type { PullRequest } from '$lib/forge/interface/types';
	import type { Readable } from 'svelte/store';
	import { page } from '$app/stores';

	const forgeListing = getForgeListingService();
	const prs = $derived<Readable<PullRequest[]> | undefined>($forgeListing?.prs);
	const pr = $derived<PullRequest | undefined>(
		$prs?.find((b) => b.number.toString() === $page.params.number)
	);
</script>

<div class="wrapper">
	<div class="inner">
		{#if !pr}
			<FullviewLoading />
		{:else if pr}
			<PullRequestPreview {pr} />
		{:else}
			<p>Branch doesn't seem to exist</p>
		{/if}
	</div>
</div>

<style lang="postcss">
	.wrapper {
		display: flex;
		height: 100%;
		overflow-y: auto;
		overscroll-behavior: none;
	}
	.inner {
		display: flex;
		padding: 16px;
	}
</style>
