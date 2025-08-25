<script lang="ts">
	import { BASE_BRANCH_SERVICE } from '$lib/baseBranch/baseBranchService.svelte';
	import { BRANCH_SERVICE } from '$lib/branches/branchService.svelte';
	import { DEFAULT_FORGE_FACTORY } from '$lib/forge/forgeFactory.svelte';
	import { inject } from '@gitbutler/shared/context';
	import { Button, TimeAgo, type ButtonProps } from '@gitbutler/ui';

	interface Props {
		projectId: string;
		size?: ButtonProps['size'];
		disabled?: boolean;
	}

	const { projectId, size = 'tag', disabled = false }: Props = $props();

	const baseBranchService = inject(BASE_BRANCH_SERVICE);
	const branchService = inject(BRANCH_SERVICE);
	const baseBranch = $derived(baseBranchService.baseBranch(projectId));

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
		Fetching...
	{:else if lastFetched}
		<span class="capitalize">
			<TimeAgo date={lastFetched} addSuffix={true} />
		</span>
	{:else}
		Refetch
	{/if}
</Button>
