<script lang="ts">
	import BaseBranchService from '$lib/baseBranch/baseBranchService.svelte';
	import { BranchService } from '$lib/branches/branchService.svelte';
	import { DefaultForgeFactory } from '$lib/forge/forgeFactory.svelte';
	import { getContext, inject } from '@gitbutler/shared/context';
	import Button, { type Props as ButtonProps } from '@gitbutler/ui/Button.svelte';
	import TimeAgo from '@gitbutler/ui/TimeAgo.svelte';

	interface Props {
		projectId: string;
		size?: ButtonProps['size'];
		disabled?: boolean;
	}

	const { projectId, size = 'tag', disabled = false }: Props = $props();

	const [baseBranchService, branchService] = inject(BaseBranchService, BranchService);
	const baseBranch = baseBranchService.baseBranch(projectId);

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
	{loading}
	{disabled}
	onmousedown={async (e: MouseEvent) => {
		e.preventDefault();
		e.stopPropagation();
		loading = true;
		try {
			await baseBranchService.fetchFromRemotes(projectId, 'modal');
			await Promise.all([
				listingService?.refresh(projectId),
				baseBranch.current.refetch(),
				branchService.refresh(projectId)
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
		<span class="text-12 text-weak">Refetch</span>
	{/if}
</Button>

<style lang="postcss">
	.sync-btn__busy-label {
		padding-left: 4px;
	}
</style>
