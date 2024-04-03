<script lang="ts">
	import BaseBranch from '$lib/components/BaseBranch.svelte';
	import FileCard from '$lib/components/FileCard.svelte';
	import FullviewLoading from '$lib/components/FullviewLoading.svelte';
	import Resizer from '$lib/components/Resizer.svelte';
	import ScrollableContainer from '$lib/components/ScrollableContainer.svelte';
	import { SETTINGS, type Settings } from '$lib/settings/userSettings';
	import { getContext, getContextStoreBySymbol } from '$lib/utils/context';
	import { BaseBranchService } from '$lib/vbranches/branchStoresCache';
	import { createSelectedFileIds, createSelectedFiles } from '$lib/vbranches/contexts';
	import { FileSelection } from '$lib/vbranches/fileSelection';
	import lscache from 'lscache';
	import { onMount } from 'svelte';
	const defaultBranchWidthRem = 30;
	const laneWidthKey = 'historyLaneWidth';
	const userSettings = getContextStoreBySymbol<Settings>(SETTINGS);

	const baseBranchService = getContext(BaseBranchService);
	const baseBranch = baseBranchService.base;

	createSelectedFileIds(new FileSelection());
	const selectedFiles = createSelectedFiles([]);

	let rsViewport: HTMLDivElement;
	let laneWidth: number;

	$: error$ = baseBranchService.error$;
	$: selected = $selectedFiles.length == 1 ? $selectedFiles[0] : undefined;

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
			{#if selected}
				<FileCard
					conflicted={selected.conflicted}
					file={selected}
					isUnapplied={false}
					readonly={true}
					on:close={() => {
						const selectedId = selected?.id;
						selectedFiles.update((fileIds) => fileIds.filter((file) => file.id != selectedId));
					}}
				/>
			{/if}
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
		padding: var(--size-12) var(--size-12) var(--size-12) var(--size-6);
		width: 50rem;
	}
	.card {
		margin: var(--size-12) var(--size-6) var(--size-12) var(--size-12);
		padding: var(--size-16);
	}
</style>
