<script lang="ts">
	import { Project } from '$lib/backend/projects';
	import BaseBranch from '$lib/components/BaseBranch.svelte';
	import FullviewLoading from '$lib/components/FullviewLoading.svelte';
	import Resizer from '$lib/components/Resizer.svelte';
	import ScrollableContainer from '$lib/components/ScrollableContainer.svelte';
	import FileCard from '$lib/file/FileCard.svelte';
	import { SETTINGS, type Settings } from '$lib/settings/userSettings';
	import { getContext, getContextStoreBySymbol } from '$lib/utils/context';
	import { BaseBranchService } from '$lib/vbranches/baseBranch';
	import { FileIdSelection } from '$lib/vbranches/fileIdSelection';
	import lscache from 'lscache';
	import { onMount, setContext } from 'svelte';
	import { writable } from 'svelte/store';

	const defaultBranchWidthRem = 30;
	const laneWidthKey = 'historyLaneWidth';
	const userSettings = getContextStoreBySymbol<Settings>(SETTINGS);

	const baseBranchService = getContext(BaseBranchService);
	const baseBranch = baseBranchService.base;
	const project = getContext(Project);

	const fileIdSelection = new FileIdSelection(project.id, writable([]));
	setContext(FileIdSelection, fileIdSelection);

	$: selectedFile = fileIdSelection.selectedFile;

	let rsViewport: HTMLDivElement;
	let laneWidth: number;

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
	<div class="base">
		<div
			class="base__left"
			bind:this={rsViewport}
			style:width={`${laneWidth || defaultBranchWidthRem}rem`}
		>
			<ScrollableContainer>
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
{/if}

<style lang="postcss">
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
		margin: 12px 6px 12px 12px;
		padding: 16px;
	}
</style>
