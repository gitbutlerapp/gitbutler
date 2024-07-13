<script lang="ts">
	import { getGitHostListingServiceStore } from '$lib/gitHost/interface/gitHostListingService';
	import Button from '$lib/shared/Button.svelte';
	import TimeAgo from '$lib/shared/TimeAgo.svelte';
	import { getContext } from '$lib/utils/context';
	import { BaseBranchService } from '$lib/vbranches/baseBranch';

	const githubService = getGitHostListingServiceStore();
	const baseBranchService = getContext(BaseBranchService);
	const baseBranch = baseBranchService.base;

	$: baseServiceBusy$ = baseBranchService.busy$;
</script>

<Button
	size="tag"
	clickable
	reversedDirection
	style="ghost"
	outline
	icon="update-small"
	help="Last fetch from upstream"
	loading={$baseServiceBusy$}
	on:mousedown={async (e) => {
		e.preventDefault();
		e.stopPropagation();
		await baseBranchService.fetchFromRemotes('modal');
		await $githubService?.reload();
	}}
>
	{#if $baseServiceBusy$}
		<div class="sync-btn__busy-label">busy…</div>
	{:else if $baseBranch?.lastFetched}
		<TimeAgo date={$baseBranch?.lastFetched} />
	{/if}
</Button>

<style lang="postcss">
	.sync-btn__busy-label {
		padding-left: 4px;
	}
</style>
