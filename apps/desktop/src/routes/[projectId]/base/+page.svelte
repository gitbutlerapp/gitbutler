<script lang="ts">
	import BaseBranch from '$components/BaseBranch.svelte';
	import ScrollableContainer from '$components/ConfigurableScrollableContainer.svelte';
	import FileCard from '$components/FileCard.svelte';
	import FullviewLoading from '$components/FullviewLoading.svelte';
	import Resizer from '$components/Resizer.svelte';
	import BaseBranchService from '$lib/baseBranch/baseBranchService.svelte';
	import { Project } from '$lib/project/project';
	import { FileIdSelection } from '$lib/selection/fileIdSelection';
	import { getContext } from '@gitbutler/shared/context';
	import { setContext } from 'svelte';

	const project = getContext(Project);
	const baseBranchService = getContext(BaseBranchService);
	const baseBranchResponse = $derived(baseBranchService.baseBranch(project.id));
	const baseBranch = $derived(baseBranchResponse.current.data);
	const baseBranchError = $derived(baseBranchResponse.current.error);

	const fileIdSelection = new FileIdSelection();
	setContext(FileIdSelection, fileIdSelection);

	const selectedFile = fileIdSelection.selectedFile;

	const commitId = $derived($selectedFile?.commitId);
	const selected = $derived($selectedFile?.file);

	let rsViewport = $state<HTMLDivElement>();
</script>

{#if baseBranchError}
	<p>Error...</p>
{:else if !baseBranch}
	<FullviewLoading />
{:else}
	<div class="base">
		<div class="base__left" bind:this={rsViewport}>
			<ScrollableContainer>
				<div class="card">
					<BaseBranch base={baseBranch} />
				</div>
			</ScrollableContainer>
			<Resizer
				viewport={rsViewport}
				persistId="baseLaneWidth"
				direction="right"
				minWidth={20}
				defaultValue={20}
			/>
		</div>
		<div class="base__right">
			{#if selected}
				<FileCard
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
		position: relative;
		flex-grow: 0;
		flex-shrink: 0;
		overflow-x: hidden;
	}
	.base__right {
		display: flex;
		align-items: flex-start;
		width: 800px;
		padding: 12px 12px 12px 6px;
		overflow-x: auto;
	}
	.card {
		margin: 12px 6px 12px 12px;
		border-radius: var(--radius-m);
	}
</style>
