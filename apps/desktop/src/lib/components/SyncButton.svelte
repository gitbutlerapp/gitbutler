<script lang="ts">
	import { BaseBranchService } from '$lib/baseBranch/baseBranchService';
	import { BranchListingService } from '$lib/branches/branchListing';
	import { getForgeListingService } from '$lib/forge/interface/forgeListingService';
	import { getContext } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import TimeAgo from '@gitbutler/ui/TimeAgo.svelte';

	const baseBranchService = getContext(BaseBranchService);
	const baseBranch = baseBranchService.base;
	const branchListingService = getContext(BranchListingService);

	const listingService = getForgeListingService();

	let loading = $state(false);
</script>

<Button
	size="tag"
	clickable
	reversedDirection
	style="ghost"
	outline
	icon="update-small"
	tooltip="Last fetch from upstream"
	{loading}
	onmousedown={async (e: MouseEvent) => {
		e.preventDefault();
		e.stopPropagation();
		loading = true;
		try {
			await baseBranchService.fetchFromRemotes('modal');
			await Promise.all([
				$listingService?.refresh(),
				baseBranchService.refresh(),
				branchListingService.refresh()
			]);
		} finally {
			loading = false;
		}
	}}
>
	{#if loading}
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
