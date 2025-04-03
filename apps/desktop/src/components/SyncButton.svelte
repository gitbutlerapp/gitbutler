<script lang="ts">
	import BaseBranchService from '$lib/baseBranch/baseBranchService.svelte';
	import { BranchListingService } from '$lib/branches/branchListing';
	import { DefaultForgeFactory } from '$lib/forge/forgeFactory.svelte';
	import { getContext, inject } from '@gitbutler/shared/context';
	import Button, { type Props as ButtonProps } from '@gitbutler/ui/Button.svelte';
	import TimeAgo from '@gitbutler/ui/TimeAgo.svelte';

	interface Props {
		projectId: string;
		size?: ButtonProps['size'];
	}

	const { projectId, size = 'tag' }: Props = $props();

	const [baseBranchService, branchListingService] = inject(BaseBranchService, BranchListingService);
	const baseBranch = baseBranchService.baseBranch(projectId);
	const [fetchFromRemotes] = baseBranchService.fetchFromRemotes;

	const forge = getContext(DefaultForgeFactory);
	const listingService = $derived(forge.current.listService);

	const lastFetched = $derived(baseBranch.current.data?.lastFetched);

	let loading = $state(false);
</script>

<Button
	{size}
	reversedDirection
	kind="outline"
	icon="update"
	tooltip="Last fetch from upstream"
	disabled={!lastFetched}
	{loading}
	onmousedown={async (e: MouseEvent) => {
		e.preventDefault();
		e.stopPropagation();
		loading = true;
		try {
			await fetchFromRemotes({
				projectId,
				action: 'modal'
			});
			await Promise.all([
				listingService?.refresh(projectId),
				baseBranch.current.refetch(),
				branchListingService.refresh()
			]);
		} finally {
			loading = false;
		}
	}}
>
	{#if loading}
		<div class="sync-btn__busy-label">busy…</div>
	{:else if lastFetched}
		<TimeAgo date={lastFetched} addSuffix={true} />
	{:else}
		<span class="text-12 text-weak">Could not fetch</span>
	{/if}
</Button>

<style lang="postcss">
	.sync-btn__busy-label {
		padding-left: 4px;
	}
</style>
