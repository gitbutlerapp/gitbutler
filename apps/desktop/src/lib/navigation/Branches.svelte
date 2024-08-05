<script lang="ts">
	import noBranchesSvg from '$lib/assets/empty-state/no-branches.svg?raw';
	import { BranchListingService } from '$lib/branches/branchListing';
	import { getGitHostListingService } from '$lib/gitHost/interface/gitHostListingService';
	import BranchListingSidebarEntry from '$lib/navigation/BranchListingSidebarEntry.svelte';
	import PullRequestSidebarEntry from '$lib/navigation/PullRequestSidebarEntry.svelte';
	import { getEntryUpdatedDate, type SidebarEntrySubject } from '$lib/navigation/types';
	import { persisted } from '$lib/persisted/persisted';
	import ScrollableContainer from '$lib/shared/ScrollableContainer.svelte';
	import { getContext } from '$lib/utils/context';
	import Segment from '@gitbutler/ui/SegmentControl/Segment.svelte';
	import SegmentControl from '@gitbutler/ui/SegmentControl/SegmentControl.svelte';
	import Badge from '@gitbutler/ui/shared/Badge.svelte';
	import type { PullRequest } from '$lib/gitHost/interface/types';

	const gitHostListingService = getGitHostListingService();
	const pullRequestsStore = $derived($gitHostListingService?.prs);
	const pullRequests = $derived($pullRequestsStore || []);

	const branchListingService = getContext(BranchListingService);
	const branchListings = branchListingService.branchListings;

	let sidebarEntries = $state<SidebarEntrySubject[]>([]);

	$effect(() => {
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

	let viewport = $state<HTMLDivElement>();
	let contents = $state<HTMLDivElement>();

	const selectedOption = persisted<string>('all', 'branches-selectedOption');
	const selectedIndex = $derived(
		['all', 'pullRequest', 'local'].findIndex((option) => $selectedOption === option)
	);

	function setFilter(option: string) {
		$selectedOption = option;
	}

	function filterSidebarEntries(
		pullRequests: PullRequest[],
		selectedOption: string,
		sidebarEntries: SidebarEntrySubject[]
	): SidebarEntrySubject[] {
		switch (selectedOption) {
			case 'pullRequest': {
				return sidebarEntries.filter(
					(sidebarEntry) =>
						sidebarEntry.type === 'pullRequest' ||
						pullRequests.some(
							(pullRequest) => pullRequest.sourceBranch === sidebarEntry.subject.name
						)
				);
			}
			case 'local': {
				return sidebarEntries.filter(
					(sidebarEntry) =>
						sidebarEntry.type === 'branchListing' &&
						(sidebarEntry.subject.hasLocal || sidebarEntry.subject.virtualBranch)
				);
			}
			default: {
				return sidebarEntries;
			}
		}
	}

	const searchedBranches = $derived(
		filterSidebarEntries(pullRequests, $selectedOption, sidebarEntries)
	);
	const groupedBranches = $derived(groupByDate(searchedBranches));
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
					<BranchListingSidebarEntry branchListing={sidebarEntrySubject.subject} />
				{:else}
					<PullRequestSidebarEntry pullRequest={sidebarEntrySubject.subject} />
				{/if}
			{/each}
		</div>
	{/if}
{/snippet}

<div class="branches">
	<div class="header">
		<div class="branches-title">
			<span class="text-base-14 text-bold">Branches</span>

			{#if searchedBranches.length > 0}
				<Badge count={searchedBranches.length} />
			{/if}
		</div>
		<SegmentControl fullWidth defaultIndex={selectedIndex} onselect={setFilter}>
			<Segment id="all">All</Segment>
			<Segment id="pullRequest">PRs</Segment>
			<Segment id="local">Local</Segment>
		</SegmentControl>
	</div>
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

	.header {
		padding: 14px;
	}

	.branches-title {
		display: flex;
		align-items: center;
		gap: 4px;

		margin-bottom: 8px;
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

		&:first-child {
			& .group-header {
				padding-top: 0px;
			}
		}
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
