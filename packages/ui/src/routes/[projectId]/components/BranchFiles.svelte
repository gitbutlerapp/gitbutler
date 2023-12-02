<script lang="ts">
	import { filesToFileTree, sortLikeFileTree } from '$lib/vbranches/filetree';
	import type { Branch } from '$lib/vbranches/types';
	import { slide } from 'svelte/transition';
	import type { Ownership } from '$lib/vbranches/ownership';
	import type { Writable } from 'svelte/store';
	import Badge from '$lib/components/Badge.svelte';
	import SegmentedControl from '$lib/components/SegmentControl/SegmentedControl.svelte';
	import Segment from '$lib/components/SegmentControl/Segment.svelte';
	import FileListItem from './FileListItem.svelte';
	import FileTree from './FileTree.svelte';
	import type { UIEventHandler } from 'svelte/elements';
	import Scrollbar from '$lib/components/Scrollbar.svelte';
	import Resizer from '$lib/components/Resizer.svelte';
	import { onDestroy, onMount } from 'svelte';

	export let branch: Branch;
	export let readonly: boolean;
	export let selectedOwnership: Writable<Ownership>;
	export let selectedFileId: Writable<string | undefined>;

	let selectedListMode: string;

	let viewport: HTMLElement | undefined;
	let contents: HTMLElement | undefined;
	let rsViewport: HTMLElement;

	let scrolled: boolean;
	let height: number | undefined = undefined;
	let headerHeight: number;
	let headerElement: HTMLDivElement;
	let resizable = false;

	let observer: ResizeObserver;

	const onScroll: UIEventHandler<HTMLDivElement> = (e) => {
		scrolled = e.currentTarget.scrollTop != 0;
	};

	function updateResizable() {
		if (viewport && contents) {
			const oldValue = resizable;
			resizable = viewport.offsetHeight <= contents.offsetHeight;
			if (oldValue == false && resizable == true) {
				height = viewport.offsetHeight;
			}
		} else {
			resizable = false;
		}
	}

	onMount(() => {
		updateResizable();
		observer = new ResizeObserver(() => updateResizable());
		if (viewport) observer.observe(viewport);
		headerHeight = headerElement?.offsetHeight;
	});

	onDestroy(() => observer.disconnect());
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

<div class="resize-viewport" bind:this={rsViewport}>
	<div class="branch-files-wrapper">
		{#if branch.files.length > 0}
			<div class="header" class:border-b={scrolled} bind:this={headerElement}>
				<div class="text-bold">
					Changes <Badge count={branch.files.length} />
				</div>
				<SegmentedControl bind:selected={selectedListMode} selectedIndex={0}>
					<Segment id="list" icon="list-view" />
					<Segment id="tree" icon="tree-view" />
				</SegmentedControl>
			</div>
			<div class="scrollbar-container">
				<div
					class="files-viewport hide-native-scrollbar"
					style:height={resizable ? `${height}px` : undefined}
					style:min-height={`${2 * headerHeight}px`}
					bind:this={viewport}
					transition:slide={{ duration: readonly ? 0 : 250 }}
					on:scroll={onScroll}
				>
					<!-- TODO: This is an experiment in file sorting. Accept or reject! -->
					<div class="files-content" bind:this={contents}>
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
				</div>
				<Scrollbar {viewport} {contents} thickness="0.4rem" />
			</div>
		{/if}
	</div>
	{#if resizable && viewport}
		<Resizer
			inside
			direction="down"
			{viewport}
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
		min-height: 8rem;
		flex-shrink: 0;
		flex-grow: 1;
	}
	.branch-files-wrapper {
		display: flex;
		flex-direction: column;
		flex-shrink: 0;
		flex-grow: 1;
		overflow: hidden;
		max-height: 100%;
	}
	.header {
		color: var(----clr-theme-scale-ntrl-0);
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding-top: var(--space-12);
		padding-bottom: var(--space-12);
		padding-left: var(--space-16);
		padding-right: var(--space-12);
		border-color: var(--clr-theme-container-outline-light);
	}
	.scrollbar-container {
		position: relative;
		display: flex;
		flex-direction: column;
		overflow: hidden;
		flex-grow: 1;
		width: 100%;
	}
	.files-viewport {
		flex-grow: 1;
		padding-top: 0;
		padding-left: var(--space-12);
		padding-right: var(--space-12);
		overflow-y: scroll;
		overflow-x: hidden;
		overscroll-behavior: none;
	}

	.files-content {
		display: flex;
		flex-direction: column;
		flex-grow: 1;
		padding-bottom: var(--space-16);
		gap: var(--space-4);
	}
</style>
