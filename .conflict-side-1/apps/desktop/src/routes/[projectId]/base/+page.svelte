<script lang="ts">
	import BaseBranch from '$components/BaseBranch.svelte';
	import ScrollableContainer from '$components/ConfigurableScrollableContainer.svelte';
	import FileCard from '$components/FileCard.svelte';
	import FullviewLoading from '$components/FullviewLoading.svelte';
	import Resizer from '$components/Resizer.svelte';
	import { BaseBranchService } from '$lib/baseBranch/baseBranchService';
	import { FileIdSelection } from '$lib/selection/fileIdSelection';
	import { getContext } from '@gitbutler/shared/context';
	import { persisted } from '@gitbutler/shared/persisted';
	import { setContext } from 'svelte';

	const laneWidthKey = 'baseLaneWidth';
	const width = persisted<number>(20, laneWidthKey);

	const baseBranchService = getContext(BaseBranchService);
	const baseBranch = baseBranchService.base;

	const fileIdSelection = new FileIdSelection();
	setContext(FileIdSelection, fileIdSelection);

	const selectedFile = fileIdSelection.selectedFile;

	const commitId = $derived($selectedFile?.commitId);
	const selected = $derived($selectedFile?.file);

	let rsViewport = $state<HTMLDivElement>();

	const error = baseBranchService.error;
</script>

{#if $error}
	<p>Error...</p>
{:else if !$baseBranch}
	<FullviewLoading />
{:else}
	<div class="base">
		<div class="base__left" bind:this={rsViewport} style:width={$width + 'rem'}>
			<ScrollableContainer>
				<div class="card">
					<BaseBranch base={$baseBranch} />
				</div>
			</ScrollableContainer>
			<Resizer
				viewport={rsViewport}
				direction="right"
				minWidth={20}
				onWidth={(value) => ($width = value)}
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
