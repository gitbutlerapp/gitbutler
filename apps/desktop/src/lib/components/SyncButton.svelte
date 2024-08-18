<script lang="ts">
	import { BaseBranchService } from '$lib/baseBranch/baseBranchService';
	import { BranchListingService } from '$lib/branches/branchListing';
	import { getGitHostListingService } from '$lib/gitHost/interface/gitHostListingService';
	import { getContext } from '$lib/utils/context';
	import { VirtualBranchService } from '$lib/vbranches/virtualBranch';
	import Button from '@gitbutler/ui/Button.svelte';
	import TimeAgo from '@gitbutler/ui/TimeAgo.svelte';

	const baseBranchService = getContext(BaseBranchService);
	const vbranchService = getContext(VirtualBranchService);
	const baseBranch = baseBranchService.base;
	const branchListingService = getContext(BranchListingService);

	const listingService = getGitHostListingService();

	let loading = $state(false);
</script>

<Button
	size="tag"
	clickable
	reversedDirection
	style="ghost"
	outline
	icon="update-small"
	help="Last fetch from upstream"
	{loading}
	onmousedown={async (e) => {
		e.preventDefault();
		e.stopPropagation();
		loading = true;
		try {
			await baseBranchService.fetchFromRemotes('modal');
			await Promise.all([
				$listingService?.refresh(),
				vbranchService.refresh(),
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
