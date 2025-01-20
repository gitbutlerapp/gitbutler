<script lang="ts">
	import { BaseBranchService } from '$lib/baseBranch/baseBranchService';
	import { BranchListingService } from '$lib/branches/branchListing';
	import { getForgeListingService } from '$lib/forge/interface/forgeListingService';
	import { getContext } from '@gitbutler/shared/context';
	import Button, { type Props as ButtonProps } from '@gitbutler/ui/Button.svelte';
	import TimeAgo from '@gitbutler/ui/TimeAgo.svelte';

	interface Props {
		size?: ButtonProps['size'];
	}

	const { size = 'tag' }: Props = $props();

	const baseBranchService = getContext(BaseBranchService);
	const baseBranch = baseBranchService.base;
	const branchListingService = getContext(BranchListingService);

	const listingService = getForgeListingService();

	let loading = $state(false);
</script>

<Button
	{size}
	reversedDirection
	kind="outline"
	icon="update"
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
