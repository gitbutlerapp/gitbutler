<script lang="ts">
	import ScrollableContainer from '$components/ConfigurableScrollableContainer.svelte';
	import LazyList from '$components/LazyList.svelte';
	import BranchesListGroup from '$components/branchesPage/BranchesListGroup.svelte';
	import noBranchesSvg from '$lib/assets/empty-state/no-branches.svg?raw';
	import {
		combineBranchesAndPrs,
		groupBranches,
		isBranchFilterOption,
		type BranchFilterOption,
		type SidebarEntrySubject
	} from '$lib/branches/branchListing';
	import { BRANCH_SERVICE } from '$lib/branches/branchService.svelte';
	import { DEFAULT_FORGE_FACTORY } from '$lib/forge/forgeFactory.svelte';
	import { debounce } from '$lib/utils/debounce';
	import { inject } from '@gitbutler/core/context';
	import { Badge, Button, EmptyStatePlaceholder, SegmentControl } from '@gitbutler/ui';
	import Fuse from 'fuse.js';
	import type { BaseBranch } from '$lib/baseBranch/baseBranch';
	import type { ForgeUser } from '$lib/forge/interface/types';
	import type { Snippet } from 'svelte';

	type Props = {
		projectId: string;
		forgeUser: ForgeUser | undefined;
		selectedOption: BranchFilterOption;
		sidebarEntry: Snippet<[SidebarEntrySubject]>;
		baseBranch?: BaseBranch;
	};
	let {
		projectId,
		forgeUser,
		selectedOption = $bindable(),
		sidebarEntry,
		baseBranch
	}: Props = $props();

	const searchEngine = new Fuse([] as SidebarEntrySubject[], {
		keys: [
			// Subject is branch listing
			'subject.name',
			'subject.lastCommiter.email',
			'subject.lastCommiter.name',
			'subject.stack.branches',
			// Subject is pull request
			'subject.title',
			'subject.author.email',
			'subject.author.name'
		],
		threshold: 0.3, // 0 is the strictest.
		ignoreLocation: true,
		isCaseSensitive: false,
		sortFn: (a, b) => {
			// Sort results by when the item was last modified.
			const dateA = (a.item.modifiedAt ?? a.item.updatedAt) as Date | undefined;
			const dateB = (b.item.modifiedAt ?? b.item.updatedAt) as Date | undefined;
			if (dateA !== undefined && dateB !== undefined && dateA !== dateB) {
				return dateA < dateB ? -1 : 1;
			}
			// If there are no dates or they're the same, sort by score
			return a.score < b.score ? -1 : 1;
		}
	});

	let searchTerm = $state('');
	let searchEl: HTMLInputElement | undefined = $state();
	let searching = $state(false);

	const forge = inject(DEFAULT_FORGE_FACTORY);
	const branchService = inject(BRANCH_SERVICE);

	const prs = $derived(forge.current.listService?.list(projectId, 15 * 60 * 1000));

	const branchesQuery = $derived(branchService.list(projectId));
	const combined = $derived(
		combineBranchesAndPrs(prs?.response || [], branchesQuery.response || [], selectedOption)
	);

	const groupedBranches = $derived(groupBranches(combined, forgeUser));
	const searchedBranches = $derived.by(() => {
		if (searchTerm.length >= 2 && combined.length > 0) {
			searchEngine.setCollection(combined);
			return searchEngine.search(searchTerm);
		}
		return [];
	});

	function handleSearchKeyDown(e: KeyboardEvent) {
		if (e.key === 'Escape') {
			closeSearch();
		}
	}

	function closeSearch() {
		searching = false;
		searchTerm = '';
	}

	function openSearch() {
		searching = true;
		setTimeout(() => {
			searchEl?.focus();
		}, 0);
	}

	function toggleSearch() {
		if (searching) {
			closeSearch();
		} else {
			openSearch();
		}
	}

	const filterOptions = $derived.by<Partial<Record<BranchFilterOption, string>>>(() => {
		if (forge.current.listService) {
			return {
				all: 'All',
				pullRequest: 'PRs',
				local: 'Local'
			};
		} else {
			return {
				all: 'All',
				local: 'Local'
			};
		}
	});

	function setFilter(id: string) {
		if (isBranchFilterOption(id)) {
			selectedOption = id;
		}
	}

	const debounceSearchInput = debounce(() => {
		searchTerm = searchEl!.value;
	}, 250);

	// Mapping for forge type to display name
	const FORGE_NAME_MAP: Record<string, string> = {
		github: 'GitHub',
		gitlab: 'GitLab',
		bitbucket: 'Bitbucket',
		azure: 'Azure DevOps'
	};

	// Increased from 180 to accommodate longer contextual messages
	const EMPTY_STATE_WIDTH = 280;

	// Helper to get a user-friendly forge name
	const forgeName = $derived(FORGE_NAME_MAP[forge.determinedForgeType] ?? 'your forge');

	// Helper to determine if authentication message should be shown
	// Show auth message when:
	// 1. User is not authenticated, AND
	// 2. Either viewing PRs filter OR forge has PR listing capability
	const shouldShowAuthMessage = $derived(
		!forge.current.authenticated && (selectedOption === 'pullRequest' || forge.current.listService)
	);
</script>

{#snippet branchGroup(props: { title: string; children: SidebarEntrySubject[] })}
	{#if props.children.length > 0}
		<BranchesListGroup title={props.title}>
			<LazyList items={props.children} template={sidebarEntry}></LazyList>
		</BranchesListGroup>
	{/if}
{/snippet}

<div class="branches">
	<div class="branches__header">
		<div class="branches__header-info">
			<div class="branches-title" class:hide-branch-title={searching}>
				<span class="text-14 text-bold">Branches</span>

				<Badge>{combined.length}</Badge>
			</div>

			<div class="search-container" class:show-search={searching}>
				<div class="search-button">
					<Button
						icon={searching ? 'cross' : 'search'}
						kind="ghost"
						onclick={toggleSearch}
						tabindex={searching ? -1 : 0}
					/>
				</div>

				<input
					bind:this={searchEl}
					oninput={debounceSearchInput}
					class="search-input text-13"
					type="text"
					placeholder="Search branches"
					onkeydown={handleSearchKeyDown}
				/>
			</div>
		</div>

		<SegmentControl fullWidth selected={selectedOption} onselect={setFilter}>
			{#each Object.entries(filterOptions) as [segmentId, segmentCopy]}
				<SegmentControl.Item id={segmentId}>{segmentCopy}</SegmentControl.Item>
			{/each}
		</SegmentControl>
	</div>

	{#if combined.length > 0}
		{#if searchedBranches.length > 0 || !searchTerm}
			<div class="branch-entries-list">
				<ScrollableContainer>
					{#if searchTerm}
						<div class="group">
							{#each searchedBranches as searchResult}
								{@render sidebarEntry(searchResult.item)}
							{/each}
						</div>
					{:else}
						{@render branchGroup({ title: 'Applied', children: groupedBranches.applied })}

						{#if groupedBranches.authored.length > 0}
							{@render branchGroup({ title: 'Mine', children: groupedBranches.authored })}
						{/if}

						{#if groupedBranches.review.length > 0}
							{@render branchGroup({ title: 'Review Requested', children: groupedBranches.review })}
						{/if}

						{@render branchGroup({ title: 'Today', children: groupedBranches.today })}
						{@render branchGroup({ title: 'Yesterday', children: groupedBranches.yesterday })}
						{@render branchGroup({ title: 'Last week', children: groupedBranches.lastWeek })}
						{@render branchGroup({ title: 'Older', children: groupedBranches.older })}
					{/if}
				</ScrollableContainer>
			</div>
		{:else}
			<EmptyStatePlaceholder image={noBranchesSvg} width={180} bottomMargin={48}>
				{#snippet caption()}
					No branches<br />match your filter
				{/snippet}
			</EmptyStatePlaceholder>
		{/if}
	{:else}
		<div class="branches__empty-state">
			<EmptyStatePlaceholder image={noBranchesSvg} width={EMPTY_STATE_WIDTH} bottomMargin={48}>
				{#snippet title()}
					{#if selectedOption === 'local'}
						No local branches found
					{:else}
						No branches or {forge.reviewUnitAbbr}s found
					{/if}
				{/snippet}
				{#snippet caption()}
					{#if selectedOption === 'pullRequest'}
						No {forge.reviewUnitAbbr}s found {#if baseBranch}
							from <strong>{baseBranch.remoteName}</strong>{/if}.
					{:else if selectedOption === 'local'}
						Create a new branch or fetch from your remote.
					{:else if baseBranch}
						Branches and {forge.reviewUnitAbbr}s from
						<strong>{baseBranch.remoteName}/{baseBranch.shortName}</strong>
						will appear here.
					{/if}
					{#if shouldShowAuthMessage}
						Authenticate with {forgeName} to see {forge.reviewUnitAbbr}s.
					{/if}
				{/snippet}
			</EmptyStatePlaceholder>
		</div>
	{/if}
</div>

<style lang="postcss">
	.branches {
		display: flex;
		position: relative;
		flex: 1;
		flex-direction: column;
		height: 100%;
		overflow: hidden;
		background-color: var(--clr-bg-2);
	}

	.branches__header {
		display: flex;
		flex-direction: column;
		padding: 8px 14px 14px 14px;
	}

	.branches__header-info {
		display: flex;
		position: relative;
		align-items: center;
		justify-content: flex-end;
		width: 100%;
		height: 32px;
		margin-bottom: 8px;
	}

	.branches-title {
		display: flex;
		position: absolute;
		top: 50%;
		left: 0;
		align-items: center;
		gap: 6px;
		transform: translateY(-50%);

		transition:
			opacity 0.1s ease,
			transform 0.1s ease;
	}

	/* SEARCH */
	.search-container {
		position: relative;
		width: 60%;
		height: var(--size-cta);
		overflow: hidden;
		transition: width 0.16s ease;
	}

	.search-button {
		display: flex;
		z-index: var(--z-ground);
		position: absolute;
		top: 50%;
		right: 0;
		align-items: center;
		justify-content: center;
		transform: translateY(-50%);
	}

	.search-input {
		display: none;
		width: 100%;
		height: 100%;
		padding-left: 8px;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-s) var(--radius-m) var(--radius-m) var(--radius-s);
		background-color: var(--clr-bg-1);
		transition: opacity 0.1s;

		&:focus-within {
			outline: none;
		}

		&:hover,
		&:focus {
			border-color: var(--clr-border-1);
		}

		&::placeholder {
			color: var(--clr-text-3);
		}
	}

	.show-search {
		width: 100%;

		& .search-button::after {
			transform: scale(0.7);
		}

		& .search-input {
			display: block;
		}
	}

	.hide-branch-title {
		transform: translateX(-5px) translateY(-50%);
		opacity: 0;
	}

	/* BRANCHES LIST */
	.branch-entries-list {
		width: 100%;
		margin-top: -1px;
		overflow: hidden;
		border-top: 1px solid var(--clr-border-2);
	}

	.group {
		display: flex;
		flex-direction: column;

		&:last-child {
			margin-bottom: 0;
			border-bottom: none;
		}
	}

	/* EMPTY STATE */
	.branches__empty-state {
		display: flex;
		flex: 1;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: 10px;
	}
</style>
