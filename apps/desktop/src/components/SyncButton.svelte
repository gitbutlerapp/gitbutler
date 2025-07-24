<script lang="ts">
	import { BASE_BRANCH_SERVICE } from '$lib/baseBranch/baseBranchService.svelte';
	import { BRANCH_SERVICE } from '$lib/branches/branchService.svelte';
	import { DEFAULT_FORGE_FACTORY } from '$lib/forge/forgeFactory.svelte';
	import { inject } from '@gitbutler/shared/context';
	import Button, { type Props as ButtonProps } from '@gitbutler/ui/Button.svelte';
	import TimeAgo from '@gitbutler/ui/TimeAgo.svelte';

	interface Props {
		projectId: string;
		size?: ButtonProps['size'];
		disabled?: boolean;
	}

	const { projectId, size = 'tag', disabled = false }: Props = $props();

	const baseBranchService = inject(BASE_BRANCH_SERVICE);
	const branchService = inject(BRANCH_SERVICE);
	const baseBranch = baseBranchService.baseBranch(projectId);

	const forge = inject(DEFAULT_FORGE_FACTORY);
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
				branchService.refresh()
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
