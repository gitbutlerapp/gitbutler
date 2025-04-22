<script lang="ts">
	import BranchListingSidebarEntry from '$components/BranchListingSidebarEntry.svelte';
	import ChunkyList from '$components/ChunkyList.svelte';
	import ScrollableContainer from '$components/ConfigurableScrollableContainer.svelte';
	import GroupHeader from '$components/GroupHeader.svelte';
	import PullRequestSidebarEntry from '$components/PullRequestSidebarEntry.svelte';
	import noBranchesSvg from '$lib/assets/empty-state/no-branches.svg?raw';
	import {
		combineBranchesAndPrs,
		groupBranches,
		type SidebarEntrySubject
	} from '$lib/branches/branchListing';
	import { BranchService } from '$lib/branches/branchService.svelte';
	import { DefaultForgeFactory } from '$lib/forge/forgeFactory.svelte';
	import { inject } from '@gitbutler/shared/context';
	import { persisted } from '@gitbutler/shared/persisted';
	import Badge from '@gitbutler/ui/Badge.svelte';
	import EmptyStatePlaceholder from '@gitbutler/ui/EmptyStatePlaceholder.svelte';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import Segment from '@gitbutler/ui/segmentControl/Segment.svelte';
	import SegmentControl from '@gitbutler/ui/segmentControl/SegmentControl.svelte';
	import Fuse from 'fuse.js';

	const { projectId }: { projectId: string } = $props();

	const selectedOption = persisted<'all' | 'pullRequest' | 'local'>(
		'all',
		`branches-selectedOption-${projectId}`
	);

	const searchEngine = new Fuse([] as SidebarEntrySubject[], {
		keys: [
			// Subject is branch listing
			'subject.name',
			'subject.lastCommiter.email',
			'subject.lastCommiter.name',
			'subject.virtualBranch.stackBranches',
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

	const [forge, branchService] = inject(DefaultForgeFactory, BranchService);

	const pollingInterval = 15 * 60 * 1000; // 15 minutes.
	const prListResult = $derived(forge.current.listService?.list(projectId, pollingInterval));

	const branchesResult = $derived(branchService.list(projectId));
	const combined = $derived(
		combineBranchesAndPrs(
			prListResult?.current.data || [],
			branchesResult.current.data || [],
			$selectedOption
		)
	);
	const groupedBranches = $derived(groupBranches(combined));
	const searchedBranches = $derived(searchTerm ? searchEngine.search(searchTerm) : []);

	$effect(() => {
		searchEngine.setCollection(combined);
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

	const filterOptions = $derived.by(() => {
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

	const selectedFilterIndex = $derived.by(() => {
		const index = Object.keys(filterOptions).findIndex((item) => $selectedOption === item);
		if (index === -1) return 0;
		return index;
	});

	function setFilter(id: string) {
		if (Object.keys(filterOptions).includes(id)) {
			// Not a fan of this
			$selectedOption = id as 'all' | 'pullRequest' | 'local';
		}
	}
</script>

{#snippet sidebarEntry(sidebarEntrySubject: SidebarEntrySubject)}
	{#if sidebarEntrySubject.type === 'branchListing'}
		<BranchListingSidebarEntry
			{projectId}
			branchListing={sidebarEntrySubject.subject}
			prs={sidebarEntrySubject.prs}
		/>
	{:else}
		<PullRequestSidebarEntry pullRequest={sidebarEntrySubject.subject} />
	{/if}
{/snippet}

{#snippet branchGroup(props: { title: string; children: SidebarEntrySubject[] })}
	{#if props.children.length > 0}
		<div class="group">
			<GroupHeader title={props.title} />
			<ChunkyList items={props.children} item={sidebarEntry}></ChunkyList>
		</div>
	{/if}
{/snippet}

<div class="branches">
	<div class="header">
		<div class="header-info">
			<div class="branches-title" class:hide-branch-title={searching}>
				<span class="text-14 text-bold">Branches</span>

				<Badge>{combined.length}</Badge>
			</div>

			<div class="search-container" class:show-search={searching}>
				<button
					type="button"
					tabindex={searching ? -1 : 0}
					class="search-button"
					onclick={toggleSearch}
				>
					<Icon name={searching ? 'cross' : 'search'} />
				</button>

				<input
					bind:this={searchEl}
					bind:value={searchTerm}
					class="search-input text-13"
					type="text"
					placeholder="Search branches"
					onkeydown={handleSearchKeyDown}
				/>
			</div>
		</div>

		<SegmentControl fullWidth defaultIndex={selectedFilterIndex} onselect={setFilter}>
			{#each Object.entries(filterOptions) as [segmentId, segmentCopy]}
				<Segment id={segmentId} disabled={!!searchTerm}>{segmentCopy}</Segment>
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
								{@const sidebarEntrySubject = searchResult.item}
								{#if sidebarEntrySubject.type === 'branchListing'}
									<BranchListingSidebarEntry
										{projectId}
										branchListing={sidebarEntrySubject.subject}
										prs={sidebarEntrySubject.prs}
									/>
								{:else}
									<PullRequestSidebarEntry pullRequest={sidebarEntrySubject.subject} />
								{/if}
							{/each}
						</div>
					{:else}
						{@render branchGroup({ title: 'Applied', children: groupedBranches.applied })}
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
			<div class="branches__empty-state__image">
				{@html noBranchesSvg}
			</div>
			<span class="branches__empty-state__caption text-14 text-body text-semibold"
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
		border-bottom: 1px solid var(--clr-border-3);
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
		display: none;
		padding-left: 8px;
		border-radius: var(--radius-s);
		border: 1px solid var(--clr-border-2);
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
			color: var(--clr-scale-ntrl-60);
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
		opacity: 0;
		transform: translateX(-5px);
	}

	/* BRANCHES LIST */
	.branch-entries-list {
		margin-top: -1px;
		overflow: hidden;
		width: 100%;
	}

	.group {
		display: flex;
		flex-direction: column;
		/* border-bottom: 1px solid var(--clr-border-3); */
		/* margin-bottom: 12px; */

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
