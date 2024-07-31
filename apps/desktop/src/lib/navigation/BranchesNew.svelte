<script lang="ts">
	// import BranchItemNew from './BranchItemNew.svelte';
	import BranchesHeaderNew from './BranchesHeaderNew.svelte';
	import noBranchesSvg from '$lib/assets/empty-state/no-branches.svg?raw';
	import { BranchListing, BranchListingService } from '$lib/branches/branchListing';
	import SmartSidebarEntry from '$lib/navigation/SmartSidebarEntry.svelte';
	import ScrollableContainer from '$lib/shared/ScrollableContainer.svelte';
	import { getContext } from '$lib/utils/context';

	const branchListingService = getContext(BranchListingService);
	// const gitHostListingService = getGitHostListingService();

	const branchesStore = branchListingService.branchListings;
	const branches = $derived($branchesStore || []);
	const searchedBranches = $derived(branches);

	function searchMatchesAnIdentifier(search: string, identifiers: string[]) {
		for (const identifier of identifiers) {
			if (identifier.includes(search.toLowerCase())) return true;
		}

		return false;
	}

	const oneDay = 1000 * 60 * 60 * 24;
	function groupByDate(branches: BranchListing[]) {
		const grouped: Record<'today' | 'yesterday' | 'lastWeek' | 'older', BranchListing[]> = {
			today: [],
			yesterday: [],
			lastWeek: [],
			older: []
		};

		const now = Date.now();

		branches.forEach((b) => {
			if (!b.updatedAt) {
				grouped.older.push(b);
				return;
			}

			const msSinceLastCommit = now - b.updatedAt.getTime();

			if (msSinceLastCommit < oneDay) {
				grouped.today.push(b);
			} else if (msSinceLastCommit < 2 * oneDay) {
				grouped.yesterday.push(b);
			} else if (msSinceLastCommit < 7 * oneDay) {
				grouped.lastWeek.push(b);
			} else {
				grouped.older.push(b);
			}
		});

		return grouped;
	}

	const groupedBranches = $derived(groupByDate(searchedBranches));

	let viewport = $state<HTMLDivElement>();
	let contents = $state<HTMLDivElement>();
</script>

{#snippet branchGroup(props: {
	title: string,
	children: BranchListing[]
})}
	{#if props.children.length > 0}
		<div class="group">
			<h3 class="text-base-12 text-semibold group-header">{props.title}</h3>
			{#each props.children as branchListing}
				<SmartSidebarEntry {branchListing} />
			{/each}
		</div>
	{/if}
{/snippet}

<div class="branches">
	<BranchesHeaderNew
		totalBranchCount={branches.length}
		filteredBranchCount={searchedBranches?.length}
		onSearch={(value) => (searchValue = value)}
	>
		<!-- {#snippet filterButton()}
			<FilterButton
				{filtersActive}
				{includePrs}
				{includeRemote}
				{hideBots}
				{hideInactive}
				showPrCheckbox={!!$gitHost}
				on:action
			/>
		{/snippet} -->
	</BranchesHeaderNew>
	{#if branches.length > 0}
		{#if searchedBranches.length > 0}
			<ScrollableContainer
				bind:viewport
				bind:contents
				showBorderWhenScrolled
				fillViewport={searchedBranches.length === 0}
			>
				<div bind:this={contents} class="scroll-container">
					{@render branchGroup({ title: 'Today', children: groupedBranches.today })}
					{@render branchGroup({ title: 'Yesterday', children: groupedBranches.yesterday })}
					{@render branchGroup({ title: 'Last week', children: groupedBranches.lastWeek })}
					{@render branchGroup({ title: 'Older', children: groupedBranches.older })}
				</div>
			</ScrollableContainer>
		{:else}
			<div class="branches__empty-state">
				<div class="branches__empty-state__image">
					{@html noBranchesSvg}
				</div>
				<span class="branches__empty-state__caption text-base-body-14 text-semibold"
					>No branches match your filter</span
				>
			</div>
		{/if}
	{:else}
		<div class="branches__empty-state">
			<div class="branches__empty-state__image">
				{@html noBranchesSvg}
			</div>
			<span class="branches__empty-state__caption text-base-body-14 text-semibold"
				>You have no branches</span
			>
		</div>
	{/if}
</div>

<style lang="postcss">
	.branches {
		flex: 1;
		position: relative;
		overflow: hidden;
		display: flex;
		flex-direction: column;
		border-top: 1px solid var(--clr-border-2);
	}

	/* BRANCHES LIST */

	.scroll-container {
		display: flex;
		flex-direction: column;
	}

	.group {
		display: flex;
		flex-direction: column;
		border-bottom: 1px solid var(--clr-border-2);
	}

	.group-header {
		padding: 20px 14px 4px;
		color: var(--clr-text-3);
	}

	/* EMPTY STATE */
	.branches__empty-state {
		flex: 1;
		display: flex;
		flex-direction: column;
		justify-content: center;
		align-items: center;
		gap: 10px;
	}

	.branches__empty-state__image {
		width: 130px;
	}

	.branches__empty-state__caption {
		color: var(--clr-scale-ntrl-60);
		text-align: center;
		max-width: 160px;
	}
</style>
