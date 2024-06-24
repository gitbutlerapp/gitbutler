<script lang="ts">
	import RemoteCommitList from './RemoteCommitList.svelte';
	import { Project } from '$lib/backend/projects';
	import BranchPreviewHeader from '$lib/branch/BranchPreviewHeader.svelte';
	import FileCard from '$lib/file/FileCard.svelte';
	import SearchBar from '$lib/searchBar/SearchBar.svelte';
	import { SETTINGS, type Settings } from '$lib/settings/userSettings';
	import Resizer from '$lib/shared/Resizer.svelte';
	import ScrollableContainer from '$lib/shared/ScrollableContainer.svelte';
	import { getRemoteBranchData } from '$lib/stores/remoteBranches';
	import { getContext, getContextStore, getContextStoreBySymbol } from '$lib/utils/context';
	import { FileIdSelection } from '$lib/vbranches/fileIdSelection';
	import { REMOTE_BRANCH_FILTERS, type AppliedFilter } from '$lib/vbranches/filtering';
	import { BaseBranch, type RemoteBranch } from '$lib/vbranches/types';
	import lscache from 'lscache';
	import { marked } from 'marked';
	import { onMount, setContext } from 'svelte';
	import { writable } from 'svelte/store';
	import type { PullRequest } from '$lib/github/types';

	const filterDescriptions = REMOTE_BRANCH_FILTERS;

	export let branch: RemoteBranch;
	export let pr: PullRequest | undefined;

	const project = getContext(Project);
	const baseBranch = getContextStore(BaseBranch);

	const fileIdSelection = new FileIdSelection(project.id, writable([]));
	setContext(FileIdSelection, fileIdSelection);

	$: selectedFile = fileIdSelection.selectedFile;

	const defaultBranchWidthRem = 30;
	const laneWidthKey = 'branchPreviewLaneWidth';
	const userSettings = getContextStoreBySymbol<Settings>(SETTINGS);

	let rsViewport: HTMLDivElement;
	let laneWidth: number;
	let searchQuery: string | undefined = undefined;
	let searchFilters: AppliedFilter[] = [];
	let commitListElem: RemoteCommitList;

	$: filtersApplied = searchFilters.length > 0 || searchQuery;

	// Reset the search query and filters when the branch changes
	$: if (branch) {
		searchQuery = undefined;
		searchFilters = [];
	}

	onMount(() => {
		laneWidth = lscache.get(laneWidthKey);
	});

	var renderer = new marked.Renderer();
	renderer.link = function (href, title, text) {
		if (!title) title = text;
		return '<a target="_blank" href="' + href + '" title="' + title + '">' + text + '</a>';
	};
</script>

<div class="container">
	<div class="search">
		<SearchBar
			bind:value={searchQuery}
			bind:appliedFilters={searchFilters}
			{filterDescriptions}
			icon="search"
			placeholder="Search"
		/>
	</div>
	<div class="base">
		<div
			class="base__left"
			bind:this={rsViewport}
			style:width={`${laneWidth || defaultBranchWidthRem}rem`}
		>
			<ScrollableContainer wide>
				<div class="branch-preview">
					<BranchPreviewHeader base={$baseBranch} {branch} {pr} />
					{#if pr?.body && !filtersApplied}
						<div class="card">
							<div class="card__header text-base-body-14 text-semibold">PR Description</div>
							<div class="markdown card__content text-base-body-13">
								{@html marked.parse(pr.body, { renderer })}
							</div>
						</div>
					{/if}
					{#await getRemoteBranchData(project.id, branch.name) then branchData}
						{#if branchData.commits && branchData.commits.length > 0}
							<RemoteCommitList
								commits={branchData.commits}
								isUnapplied={true}
								type="remote"
								getCommitUrl={(commitId) => $baseBranch?.commitUrl(commitId)}
								{searchFilters}
								{searchQuery}
							/>
						{/if}
					{/await}
					{#if filtersApplied && commitListElem?.isEmpty()}
						<div class="info-text text-base-13">No commits found that match the current search</div>
					{/if}
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
</div>

<style lang="postcss">
	.container {
		display: flex;
		flex-direction: column;
		flex-grow: 1;
		overflow: hidden;
	}

	.search {
		padding: 12px;
		padding-bottom: 0;
	}

	.base {
		display: flex;
		flex-grow: 1;
		overflow-x: auto;
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

	.branch-preview {
		display: flex;
		flex-direction: column;
		gap: 8px;
		margin: 12px 6px 12px 12px;
	}

	.card__content {
		color: var(--clr-scale-ntrl-30);
	}

	.info-text {
		opacity: 0.5;
	}
</style>
