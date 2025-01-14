<script lang="ts">
	import BaseBranch from '$components/BaseBranch.svelte';
	import FileCard from '$components/FileCard.svelte';
	import FullviewLoading from '$components/FullviewLoading.svelte';
	import Resizer from '$components/Resizer.svelte';
	import ScrollableContainer from '$components/ScrollableContainer.svelte';
	import { BaseBranchService } from '$lib/baseBranch/baseBranchService';
	import { SETTINGS, type Settings } from '$lib/settings/userSettings';
	import { FileIdSelection } from '$lib/vbranches/fileIdSelection';
	import { getContext, getContextStoreBySymbol } from '@gitbutler/shared/context';
	import lscache from 'lscache';
	import { onMount, setContext } from 'svelte';

	const defaultBranchWidthRem = 30;
	const laneWidthKey = 'historyLaneWidth';
	const userSettings = getContextStoreBySymbol<Settings>(SETTINGS);

	const baseBranchService = getContext(BaseBranchService);
	const baseBranch = baseBranchService.base;

	const fileIdSelection = new FileIdSelection();
	setContext(FileIdSelection, fileIdSelection);

	const selectedFile = fileIdSelection.selectedFile;

	const commitId = $derived($selectedFile?.commitId);
	const selected = $derived($selectedFile?.file);

	let rsViewport = $state<HTMLDivElement>();
	let laneWidth = $state<number>();

	const error = baseBranchService.error;

	onMount(() => {
		laneWidth = lscache.get(laneWidthKey);
	});
</script>

{#if $error}
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
				onWidth={(value) => {
					laneWidth = value / (16 * $userSettings.zoom);
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
					{commitId}
					onClose={() => {
						fileIdSelection.clear();
					}}
				/>
			{/if}
		</div>
	</div>
{/if}

<style lang="postcss">
	.base {
		display: flex;
		width: 100%;
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
		border-radius: var(--radius-m);
	}
</style>
