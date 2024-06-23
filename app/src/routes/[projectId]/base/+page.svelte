<script lang="ts">
	import { Project } from '$lib/backend/projects';
	import BaseBranch from '$lib/components/BaseBranch.svelte';
	import FullviewLoading from '$lib/components/FullviewLoading.svelte';
	import SearchBar from '$lib/components/SearchBar.svelte';
	import FileCard from '$lib/file/FileCard.svelte';
	import { SETTINGS, type Settings } from '$lib/settings/userSettings';
	import Resizer from '$lib/shared/Resizer.svelte';
	import ScrollableContainer from '$lib/shared/ScrollableContainer.svelte';
	import { getContext, getContextStoreBySymbol } from '$lib/utils/context';
	import { BaseBranchService } from '$lib/vbranches/baseBranch';
	import { FileIdSelection } from '$lib/vbranches/fileIdSelection';
	import { DEFAULT_FILTERS, type AppliedFilter } from '$lib/vbranches/filtering';
	import lscache from 'lscache';
	import { onMount, setContext } from 'svelte';
	import { writable } from 'svelte/store';

	const defaultBranchWidthRem = 30;
	const laneWidthKey = 'historyLaneWidth';
	const filterDescriptions = DEFAULT_FILTERS;
	const userSettings = getContextStoreBySymbol<Settings>(SETTINGS);

	const baseBranchService = getContext(BaseBranchService);
	const baseBranch = baseBranchService.base;
	const project = getContext(Project);

	const fileIdSelection = new FileIdSelection(project.id, writable([]));
	setContext(FileIdSelection, fileIdSelection);

	$: selectedFile = fileIdSelection.selectedFile;

	let rsViewport: HTMLDivElement;
	let laneWidth: number;
	let searchQuery: string | undefined = undefined;
	let searchFilters: AppliedFilter[] = [];

	$: error$ = baseBranchService.error$;

	onMount(() => {
		laneWidth = lscache.get(laneWidthKey);
	});
</script>

{#if $error$}
	<p>Error...</p>
{:else if !$baseBranch}
	<FullviewLoading />
{:else}
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
					<div class="card">
						<BaseBranch base={$baseBranch} {searchQuery} {searchFilters} />
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
{/if}

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
	.card {
		width: auto;
		margin: 12px 6px 12px 12px;
		padding: 16px;
	}
</style>
