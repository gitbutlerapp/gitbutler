<script lang="ts">
	import { filesToFileTree, sortLikeFileTree } from '$lib/vbranches/filetree';
	import type { Branch } from '$lib/vbranches/types';
	import type { Ownership } from '$lib/vbranches/ownership';
	import type { Writable } from 'svelte/store';
	import Badge from '$lib/components/Badge.svelte';
	import SegmentedControl from '$lib/components/SegmentControl/SegmentedControl.svelte';
	import Segment from '$lib/components/SegmentControl/Segment.svelte';
	import FileListItem from './FileListItem.svelte';
	import FileTree from './FileTree.svelte';
	import Resizer from '$lib/components/Resizer.svelte';
	import ScrollableContainer from '$lib/components/ScrollableContainer.svelte';

	export let branch: Branch;
	export let readonly: boolean;
	export let selectedOwnership: Writable<Ownership>;
	export let selectedFileId: Writable<string | undefined>;
	export let forceResizable = false;
	export let enableResizing = false;

	let selectedListMode: string;

	let scrollViewport: HTMLDivElement | undefined;
	let rsViewport: HTMLElement;

	let scrolled: boolean;
	let scrollable: boolean | undefined;
	let height: number | undefined = undefined;
	let maxHeight: number | undefined;
	let headerElement: HTMLDivElement;

	function updateResizable() {
		// todo
	}
</script>

{#if branch.active && branch.conflicted}
	<div class="mb-2 bg-red-500 p-2 font-bold text-white">
		{#if branch.files.some((f) => f.conflicted)}
			This virtual branch conflicts with upstream changes. Please resolve all conflicts and commit
			before you can continue.
		{:else}
			Please commit your resolved conflicts to continue.
		{/if}
	</div>
{/if}

<div class="header" class:scrolled bind:this={headerElement}>
	<div class="header-title text-base-13 text-semibold">
		<span>Changes</span>
		<Badge count={branch.files.length} />
	</div>
	<SegmentedControl bind:selected={selectedListMode} selectedIndex={0}>
		<Segment id="list" icon="list-view" />
		<Segment id="tree" icon="tree-view" />
	</SegmentedControl>
</div>
<div
	class="resize-viewport flex-grow"
	class:flex-shrink-0={(scrollable || forceResizable) && branch.commits.length > 0}
	style:min-height={scrollable || forceResizable ? `${headerElement.offsetHeight}px` : undefined}
	style:height={scrollable || forceResizable ? `${height}px` : undefined}
	style:max-height={forceResizable && maxHeight ? maxHeight + 'px' : undefined}
	bind:this={rsViewport}
>
	{#if branch.files.length > 0}
		<ScrollableContainer
			bind:viewport={scrollViewport}
			bind:maxHeight
			bind:scrollable
			bind:scrolled
		>
			<div class="scroll-container">
				<!-- TODO: This is an experiment in file sorting. Accept or reject! -->
				{#if selectedListMode == 'list'}
					{#each sortLikeFileTree(branch.files) as file (file.id)}
						<FileListItem
							{file}
							branchId={branch.id}
							{readonly}
							on:click={() => {
								if ($selectedFileId == file.id) $selectedFileId = undefined;
								else $selectedFileId = file.id;
							}}
							selected={file.id == $selectedFileId}
						/>
					{/each}
				{:else}
					<FileTree
						node={filesToFileTree(branch.files)}
						isRoot={true}
						branchId={branch.id}
						{selectedOwnership}
						{selectedFileId}
						{readonly}
					/>
				{/if}
			</div>
		</ScrollableContainer>
	{/if}
	<!-- Resizing makes no sense if there are no branch commits. -->
	{#if (forceResizable || scrollable) && enableResizing}
		<Resizer
			inside
			direction="down"
			viewport={rsViewport}
			on:height={(e) => {
				height = e.detail;
				updateResizable();
			}}
		/>
	{/if}
</div>

<style lang="postcss">
	.resize-viewport {
		position: relative;
		display: flex;
		flex-direction: column;
		overflow: hidden;
	}
	.scroll-container {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
		padding-top: 0;
		padding-left: var(--space-12);
		padding-right: var(--space-12);
		padding-bottom: var(--space-16);
	}
	.header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding-top: var(--space-12);
		padding-bottom: var(--space-12);
		padding-left: var(--space-16);
		padding-right: var(--space-12);
		border-color: var(--clr-theme-container-outline-light);
		&.scrolled {
			border-bottom: 1px solid var(--clr-theme-container-outline-light);
		}
	}
	.header-title {
		display: flex;
		align-items: center;
		gap: var(--space-4);
		color: var(--clr-theme-scale-ntrl-0);
	}
</style>
