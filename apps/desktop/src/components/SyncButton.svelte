<script lang="ts">
	import { BASE_BRANCH_SERVICE } from '$lib/baseBranch/baseBranchService.svelte';
	import { BRANCH_SERVICE } from '$lib/branches/branchService.svelte';
	import { DEFAULT_FORGE_FACTORY } from '$lib/forge/forgeFactory.svelte';
	import { inject } from '@gitbutler/shared/context';
	import { Button, TimeAgo, Icon } from '@gitbutler/ui';

	interface Props {
		projectId: string;
		disabled?: boolean;
	}

	const { projectId, disabled = false }: Props = $props();

	const baseBranchService = inject(BASE_BRANCH_SERVICE);
	const branchService = inject(BRANCH_SERVICE);
	const baseBranch = $derived(baseBranchService.baseBranch(projectId));

	const forge = inject(DEFAULT_FORGE_FACTORY);
	const listingService = $derived(forge.current.listService);

	const lastFetched = $derived(baseBranch.current.data?.lastFetched);

	let loading = $state(false);
</script>

<Button
	kind="outline"
	width="auto"
	tooltip="Last fetch from upstream"
	{loading}
	{disabled}
	icon="update"
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
	<!-- {#if loading}
		Fetching...
	{:else if lastFetched}
		<span class="capitalize">
			<TimeAgo date={lastFetched} addSuffix={true} />
		</span>
	{:else}
		Refetch
	{/if} -->

	{#snippet custom()}
		<span class="text-12 text-semibold capitalize fetch-status">
			{#if loading}
				Fetching...
			{:else if lastFetched}
				<TimeAgo date={lastFetched} addSuffix={true} />
			{:else}
				Refetch
			{/if}
		</span>

		{#if baseBranch.current.data}
			<div class="target-branch">
				<Icon name="remote-target-branch" color="var(--clr-text-2)" />
				<span class="text-12 text-semibold">
					{baseBranch.current.data.remoteName}/{baseBranch.current.data.shortName}
				</span>
			</div>
		{/if}
	{/snippet}
</Button>

<style lang="postcss">
	.fetch-status {
		padding: 0 2px;
	}

	.target-branch {
		display: inline-flex;
		align-items: center;
		gap: 4px;
		color: var(--clr-text-2);

		&:before {
			display: inline-block;
			width: 1px;
			height: 12px;
			margin: 0 2px 0 4px;
			background-color: var(--clr-text-2);
			content: '';
			opacity: 0.5;
		}
	}
</style>
