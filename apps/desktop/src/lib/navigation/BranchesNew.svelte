<script lang="ts">
	import BranchesHeaderNew from './BranchesHeaderNew.svelte';
	import noBranchesSvg from '$lib/assets/empty-state/no-branches.svg?raw';
	import { BranchListingService } from '$lib/branches/branchListing';
	import { getGitHostListingService } from '$lib/gitHost/interface/gitHostListingService';
	import SmartSidebarEntry from '$lib/navigation/BranchListingSidebarEntry.svelte';
	import PullRequestSidebarEntry from '$lib/navigation/PullRequestSidebarEntry.svelte';
	import { getEntryUpdatedDate, type SidebarEntrySubject } from '$lib/navigation/types';
	import ScrollableContainer from '$lib/shared/ScrollableContainer.svelte';
	import { getContext } from '$lib/utils/context';
	import Segment from '@gitbutler/ui/SegmentControl/Segment.svelte';
	import SegmentControl from '@gitbutler/ui/SegmentControl/SegmentControl.svelte';

	const gitHostListingService = getGitHostListingService();
	const pullRequestsStore = $derived($gitHostListingService?.prs);

	const branchListingService = getContext(BranchListingService);
	const branchListings = branchListingService.branchListings;

	let sidebarEntries = $state<SidebarEntrySubject[]>([]);
	let searchedBranches = $derived(sidebarEntries);

	$effect(() => {
		const pullRequests = $pullRequestsStore || [];

		const branchListingNames = new Set<string>(
			$branchListings.map((branchListing) => branchListing.name)
		);

		let output: SidebarEntrySubject[] = [];

		output.push(
			...pullRequests
				.filter((pullRequest) => !branchListingNames.has(pullRequest.sourceBranch))
				.map((pullRequest): SidebarEntrySubject => ({ type: 'pullRequest', subject: pullRequest }))
		);

		output.push(
			...$branchListings.map(
				(branchListing): SidebarEntrySubject => ({ type: 'branchListing', subject: branchListing })
			)
		);

		output.sort((a, b) => {
			return getEntryUpdatedDate(b).getTime() - getEntryUpdatedDate(a).getTime();
		});

		sidebarEntries = output;
	});

	const oneDay = 1000 * 60 * 60 * 24;
	function groupByDate(branches: SidebarEntrySubject[]) {
		const grouped: Record<'today' | 'yesterday' | 'lastWeek' | 'older', SidebarEntrySubject[]> = {
			today: [],
			yesterday: [],
			lastWeek: [],
			older: []
		};

		const now = Date.now();

		branches.forEach((b) => {
			if (!getEntryUpdatedDate(b)) {
				grouped.older.push(b);
				return;
			}

			const msSinceLastCommit = now - getEntryUpdatedDate(b).getTime();

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
	children: SidebarEntrySubject[]
})}
	{#if props.children.length > 0}
		<div class="group">
			<h3 class="text-base-12 text-semibold group-header">{props.title}</h3>
			{#each props.children as sidebarEntrySubject}
				{#if sidebarEntrySubject.type === 'branchListing'}
					<SmartSidebarEntry branchListing={sidebarEntrySubject.subject} />
				{:else}
					<PullRequestSidebarEntry pullRequest={sidebarEntrySubject.subject} />
				{/if}
			{/each}
		</div>
	{/if}
{/snippet}

<div class="branches">
	<BranchesHeaderNew
		totalBranchCount={$branchListings.length}
		filteredBranchCount={searchedBranches?.length}
		onSearch={(value) => (searchValue = value)}
	></BranchesHeaderNew>
	<SegmentControl fullWidth selectedIndex={0}>
		<Segment id="all">All</Segment>
		<Segment id="mine">PRs</Segment>
		<Segment id="active">Mine</Segment>
	</SegmentControl>
	{#if $branchListings.length > 0}
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
