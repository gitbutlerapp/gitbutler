<script lang="ts">
	import BranchItemNew from './BranchItemNew.svelte';
	import BranchesHeaderNew from './BranchesHeaderNew.svelte';
	import noBranchesSvg from '$lib/assets/empty-state/no-branches.svg?raw';
	import { getBranchServiceStore } from '$lib/branches/service';
	// import FilterButton from '$lib/components/FilterBranchesButton.svelte';
	// import { getGitHost } from '$lib/gitHost/interface/gitHost';
	// import { persisted } from '$lib/persisted/persisted';
	import ScrollableContainer from '$lib/shared/ScrollableContainer.svelte';
	import { readable } from 'svelte/store';
	import type { CombinedBranch } from '$lib/branches/types';

	interface Props {
		projectId: string;
	}

	const { projectId }: Props = $props();

	const branchService = getBranchServiceStore();

	let searchValue = $state<undefined | string>();
	let branches = $state($branchService?.branches || readable([]));
	let searchedBranches = $derived(filterByText($branches, searchValue));
	let groupedBranches = $derived(groupByDate(searchedBranches));

	let viewport = $state<HTMLDivElement>();
	let contents = $state<HTMLElement>();

	$effect(() => {
		branches = $branchService?.branches || readable([]);
	});

	function filterByText(branches: CombinedBranch[], searchText: string | undefined) {
		// console.log('filterByText', branches, searchText);
		if (searchText === undefined || searchText === '') return branches;

		return branches.filter((b) => searchMatchesAnIdentifier(searchText, b.searchableIdentifiers));
	}

	function searchMatchesAnIdentifier(search: string, identifiers: string[]) {
		for (const identifier of identifiers) {
			if (identifier.includes(search.toLowerCase())) return true;
		}

		return false;
	}

	function groupByDate(branches: CombinedBranch[]) {
		const grouped: Record<string, CombinedBranch[]> = {
			today: [],
			yesterday: [],
			lastWeek: [],
			older: []
		};

		const currentTs = new Date().getTime();

		const remoteBranches = branches.filter((b) => b.remoteBranch);

		remoteBranches.forEach((b) => {
			if (!b.modifiedAt) {
				grouped.older.push(b);
				return;
			}

			const modifiedAt = b.modifiedAt?.getTime();
			const ms = currentTs - modifiedAt;

			if (ms < 86400 * 1000) {
				grouped.today.push(b);
			} else if (ms < 2 * 86400 * 1000) {
				grouped.yesterday.push(b);
			} else if (ms < 7 * 86400 * 1000) {
				grouped.lastWeek.push(b);
			} else {
				grouped.older.push(b);
			}
		});

		return grouped;
	}
</script>

{#snippet branchGroup(props: {
	title: string,
	children: CombinedBranch[]

})}
	{#if props.children.length > 0}
		<div class="group">
			<h3 class="text-base-12 text-semibold group-header">{props.title}</h3>
			{#each props.children as branch}
				<BranchItemNew {projectId} {branch} />
			{/each}
		</div>
	{/if}
{/snippet}

<div class="branches">
	<BranchesHeaderNew
		totalBranchCount={$branches.length}
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
	{#if $branches.length > 0}
		{#if searchedBranches.length > 0}
			<ScrollableContainer
				bind:viewport
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
