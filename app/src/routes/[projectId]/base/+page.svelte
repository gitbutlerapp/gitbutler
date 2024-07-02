<script lang="ts">
	import { Project } from '$lib/backend/projects';
	import BaseBranch from '$lib/components/BaseBranch.svelte';
	import FullviewLoading from '$lib/components/FullviewLoading.svelte';
	import FileCard from '$lib/file/FileCard.svelte';
	import ExploreCommits from '$lib/searchBar/ExploreCommits.svelte';
	import SearchBarContainer from '$lib/searchBar/SearchBarContainer.svelte';
	import { getFilterContext } from '$lib/searchBar/filterContext.svelte';
	import { SETTINGS, type Settings } from '$lib/settings/userSettings';
	import Resizer from '$lib/shared/Resizer.svelte';
	import ScrollableContainer from '$lib/shared/ScrollableContainer.svelte';
	import { getContext, getContextStoreBySymbol } from '$lib/utils/context';
	import { BaseBranchService } from '$lib/vbranches/baseBranch';
	import { FileIdSelection } from '$lib/vbranches/fileIdSelection';
	import { FilterName, getTrunkBranchFilters } from '$lib/vbranches/filtering';
	import { VirtualBranchService } from '$lib/vbranches/virtualBranch';
	import lscache from 'lscache';
	import { onMount, setContext } from 'svelte';
	import { derived, writable } from 'svelte/store';
	import type { PageData } from './$types';

	export let data: PageData;

	const COMMITS_TO_FETCH = 500;

	const defaultBranchWidthRem = 30;
	const laneWidthKey = 'historyLaneWidth';
	const userSettings = getContextStoreBySymbol<Settings>(SETTINGS);

	const baseBranchService = getContext(BaseBranchService);
	const baseBranch = baseBranchService.base;
	const filterDescriptions = derived(baseBranch, (b) =>
		getTrunkBranchFilters({
			[FilterName.Author]: b?.recentAuthors,
			[FilterName.File]: b?.recentFiles
		})
	);
	const project = getContext(Project);
	const vbranchService = getContext(VirtualBranchService);
	const filterContext = getFilterContext();

	const activeBranches = vbranchService.activeBranches;
	const fileIdSelection = new FileIdSelection(project.id, writable([]));
	setContext(FileIdSelection, fileIdSelection);

	$: selectedFile = fileIdSelection.selectedFile;

	let rsViewport: HTMLDivElement;
	let laneWidth: number;
	let hideCommitList: boolean = true;

	$: error$ = baseBranchService.error$;

	$: if ($baseBranch?.branchName) filterContext.init($baseBranch?.branchName);

	$: ({ branches } = data.remoteBranchService);
	$: remoteBranchNames = $branches?.map((b) => b.name) ?? [];

	onMount(() => {
		laneWidth = lscache.get(laneWidthKey);
		baseBranchService.fetchLastCommits(COMMITS_TO_FETCH);
	});
</script>

{#if $error$}
	<p>Error...</p>
{:else if !$baseBranch}
	<FullviewLoading />
{:else}
	<SearchBarContainer
		filterDescriptions={$filterDescriptions}
		onFocus={() => (hideCommitList = false)}
	>
		<ExploreCommits
			bind:expanded={hideCommitList}
			filterDescriptions={$filterDescriptions}
			{remoteBranchNames}
			activeBranches={$activeBranches}
		/>
		<div class="base" class:open={!hideCommitList}>
			<div
				class="base__left"
				bind:this={rsViewport}
				style:width={`${laneWidth || defaultBranchWidthRem}rem`}
			>
				<ScrollableContainer wide>
					<div class="card">
						<BaseBranch base={$baseBranch} />
					</div>
				</ScrollableContainer>
				<Resizer
					viewport={rsViewport}
					direction="right"
					minWidth={320}
					on:width={(e) => {
						laneWidth = e.detail / (16 * $userSettings.zoom);
						lscache.set(laneWidthKey, laneWidth, 7 * 1440); // 7 day ttl
					}}
				/>
			</div>
			<div class="base__right">
				{#await $selectedFile then selected}
					{#if selected}
						<FileCard
							conflicted={selected.conflicted}
							file={selected}
							isUnapplied={false}
							readonly={true}
							on:close={() => {
								fileIdSelection.clear();
							}}
						/>
					{/if}
				{/await}
			</div>
		</div>
	</SearchBarContainer>
{/if}

<style lang="postcss">
	.base {
		z-index: var(--z-lifted);
		display: flex;
		width: 100%;
		overflow-x: auto;
		position: relative;
		top: 100vh;
		opacity: 0;
		transition:
			top var(--transition-slower),
			opacity var(--transition-slower);

		&.open {
			top: 0;
			opacity: 1;
		}
	}

	.base__left {
		display: flex;
		flex-grow: 0;
		flex-shrink: 0;
		overflow-x: hidden;
		position: relative;
	}
	.base__right {
		display: flex;
		overflow-x: auto;
		align-items: flex-start;
		padding: 12px 12px 12px 6px;
		width: 800px;
	}
	.card {
		width: auto;
		margin: 12px 6px 12px 12px;
		padding: 16px;
	}
</style>
