<script lang="ts">
	import GroupHeader from './GroupHeader.svelte';
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
	import Icon from '@gitbutler/ui/icon/Icon.svelte';
	import Badge from '@gitbutler/ui/shared/Badge.svelte';
	import Fuse from 'fuse.js';
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

	function search(searchTerm: string, sidebarEntries: SidebarEntrySubject[]) {
		const fuse = new Fuse(sidebarEntries, {
			keys: ['subject.name', 'subject.title']
		});

		return fuse.search(searchTerm).map((searchResult) => searchResult.item);
	}

	let searchEl: HTMLInputElement;
	let searching = $state(false);
	let searchTerm = $state<string>();

	const searchedBranches = $derived.by(() => {
		const filtered = filterSidebarEntries(pullRequests, $selectedOption, sidebarEntries);
		if (searchTerm) {
			return search(searchTerm, filtered);
		} else {
			return filtered;
		}
	});
	const groupedBranches = $derived(groupByDate(searchedBranches));

	function handleSearchKeyDown(e: KeyboardEvent) {
		if (e.key === 'Escape') {
			closeSearch();
		}
	}

	function closeSearch() {
		searching = false;
		searchTerm = undefined;
	}

	function openSearch() {
		searching = true;

		if (searchEl) {
			searchEl.focus();
		}
	}

	function toggleSearch() {
		if (searching) {
			closeSearch();
		} else {
			openSearch();
		}
	}
</script>

{#snippet branchGroup(props: {
	title: string,
	children: SidebarEntrySubject[]
})}
	{#if props.children.length > 0}
		<div class="group">
			<GroupHeader title={props.title} />
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
		<div class="header-info">
			<div class="branches-title" class:hide-branch-title={searching}>
				<span class="text-base-14 text-bold">Branches</span>

				{#if searchedBranches.length > 0}
					<Badge count={searchedBranches.length} />
				{/if}
			</div>

			<div class="search-container" class:show-search={searching}>
				<button tabindex={searching ? -1 : 0} class="search-button" onclick={toggleSearch}>
					<Icon name={searching ? 'cross' : 'search'} />
				</button>

				<input
					bind:this={searchEl}
					bind:value={searchTerm}
					class="search-input text-base-13"
					type="text"
					placeholder="Search branches"
					onkeydown={handleSearchKeyDown}
				/>
			</div>
		</div>

		<SegmentControl fullWidth defaultIndex={selectedIndex} onselect={setFilter}>
			<Segment id="all">All</Segment>
			<Segment id="pullRequest">PRs</Segment>
			<Segment id="local">Local</Segment>
		</SegmentControl>
	</div>

	{#if $branchListings.length > 0}
		{#if searchedBranches.length > 0}
			<ScrollableContainer bind:viewport bind:contents fillViewport={searchedBranches.length === 0}>
				<div bind:this={contents} class="scroll-container">
					{#if searchTerm}
						<div class="group">
							{#each searchedBranches as sidebarEntrySubject}
								{#if sidebarEntrySubject.type === 'branchListing'}
									<BranchListingSidebarEntry branchListing={sidebarEntrySubject.subject} />
								{:else}
									<PullRequestSidebarEntry pullRequest={sidebarEntrySubject.subject} />
								{/if}
							{/each}
						</div>
					{:else}
						{@render branchGroup({ title: 'Today', children: groupedBranches.today })}
						{@render branchGroup({ title: 'Yesterday', children: groupedBranches.yesterday })}
						{@render branchGroup({ title: 'Last week', children: groupedBranches.lastWeek })}
						{@render branchGroup({ title: 'Older', children: groupedBranches.older })}
					{/if}
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
		position: relative;
		display: flex;
		flex-direction: column;
		padding: 14px;
	}

	.header-info {
		display: flex;
		justify-content: flex-end;
		width: 100%;
		height: 32px;

		margin-bottom: 8px;
	}

	.branches-title {
		position: absolute;
		top: 22px;
		left: 14px;

		display: flex;
		align-items: center;
		gap: 4px;

		transition:
			opacity 0.1s ease,
			transform 0.1s ease;
	}

	/* SEARCH */

	.search-container {
		position: relative;
		height: var(--size-cta);
		width: 60%;
		overflow: hidden;

		transition: width 0.16s ease;
	}

	.search-button {
		z-index: var(--z-ground);
		position: absolute;
		top: 0;
		right: 0;
		height: 100%;
		width: var(--size-cta);

		display: flex;
		align-items: center;
		justify-content: center;

		color: var(--clr-scale-ntrl-50);

		&:after {
			content: '';
			position: absolute;
			z-index: -1;
			top: 0;
			left: 0;
			height: 100%;
			width: 100%;
			border-radius: var(--radius-s);
			transform-origin: center;
			transition:
				transform 0.1s ease,
				background-color 0.2s;
		}

		&:hover {
			&:after {
				background-color: var(--clr-bg-1-muted);
			}
		}
	}

	.search-input {
		width: 100%;
		height: 100%;
		opacity: 0;
		padding-left: 8px;
		border-radius: var(--radius-s);
		border: 1px solid var(--clr-border-2);
		transition: opacity 0.1s;

		&:focus-within {
			outline: none;
		}

		&::placeholder {
			color: var(--clr-scale-ntrl-60);
		}
	}

	.show-search {
		width: 100%;

		& .search-button::after {
			transform: scale(0.7);
		}

		& .search-input {
			opacity: 1;
		}
	}

	.hide-branch-title {
		opacity: 0;
		transform: translateX(-5px);
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
		margin-bottom: 12px;

		&:last-child {
			border-bottom: none;
			margin-bottom: 0;
		}
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
