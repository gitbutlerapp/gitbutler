<script lang="ts">
	import { filesToFileTree, sortLikeFileTree } from '$lib/vbranches/filetree';
	import type { Branch } from '$lib/vbranches/types';
	import { slide } from 'svelte/transition';
	import IconNewBadge from '$lib/icons/IconNewBadge.svelte';
	import type { Ownership } from '$lib/vbranches/ownership';
	import type { Writable } from 'svelte/store';
	import Badge from '$lib/components/Badge.svelte';
	import SegmentedControl from '$lib/components/SegmentControl/SegmentedControl.svelte';
	import Segment from '$lib/components/SegmentControl/Segment.svelte';
	import FileListItem from './FileListItem.svelte';
	import FileTree from './FileTree.svelte';
	import Resizer from '$lib/components/Resizer.svelte';
	import lscache from 'lscache';
	import type { UIEventHandler } from 'svelte/elements';
	import Scrollbar from '$lib/components/Scrollbar.svelte';

	export let branch: Branch;
	export let readonly: boolean;
	export let selectedOwnership: Writable<Ownership>;

	let selectedListMode: string;
	let filesHeight = 200;
	const filesHeightKey = 'filesHeight:';

	let viewport: HTMLElement;
	let contents: HTMLElement;

	let scrolled: boolean;
	const onScroll: UIEventHandler<HTMLDivElement> = (e) => {
		scrolled = e.currentTarget.scrollTop != 0;
	};

	$: scrollable = contents ? contents.scrollHeight > contents.offsetHeight : false;
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

<div class="wrapper" class:flex-grow={!scrollable}>
	{#if branch.files.length > 0}
		<div class="header" class:border-b={scrolled}>
			<div class="text-bold">
				Changes <Badge count={branch.files.length} />
			</div>
			<SegmentedControl bind:selected={selectedListMode} selectedIndex={0}>
				<Segment id="list" icon="list-view"></Segment>
				<Segment id="tree" icon="tree-view"></Segment>
			</SegmentedControl>
		</div>
		<div class="scrollbar">
			<div
				class="files hide-native-scrollbar"
				bind:this={viewport}
				style:height={scrollable ? `${filesHeight}px` : undefined}
				transition:slide={{ duration: readonly ? 0 : 250 }}
				on:scroll={onScroll}
			>
				<!-- TODO: This is an experiment in file sorting. Accept or reject! -->
				<div bind:this={contents}>
					{#if selectedListMode == 'list'}
						{#each sortLikeFileTree(branch.files) as file (file.id)}
							<FileListItem {file} branchId={branch.id} {readonly} />
							<!-- <FileCard
					expanded={file.expanded}
					conflicted={file.conflicted}
					{selectedOwnership}
					branchId={branch.id}
					{file}
					{projectPath}
					{branchController}
					{selectable}
					{readonly}
					on:expanded={(e) => {
						setExpandedWithCache(file, e.detail);
					}}
				/> -->
						{/each}
					{:else}
						<FileTree
							node={filesToFileTree(branch.files)}
							isRoot={true}
							class="p-2"
							{selectedOwnership}
						/>
					{/if}
				</div>
			</div>
			<Scrollbar {viewport} {contents} width="0.4rem" />
		</div>
		<Resizer
			minHeight={100}
			{viewport}
			direction="vertical"
			class="z-30"
			on:height={(e) => {
				filesHeight = e.detail;
				lscache.set(filesHeightKey + branch.id, e.detail, 7 * 1440); // 7 day ttl
			}}
		/>
	{/if}
	{#if branch.files.length == 0}
		{#if branch.commits.length == 0}
			<div class="new-branch text-color-3 space-y-6 rounded p-8 text-center" data-dnd-ignore>
				<p>Nothing on this branch yet.</p>
				{#if !readonly}
					<IconNewBadge class="mx-auto mt-4 h-16 w-16 text-blue-400" />
					<p class="px-12">Get some work done, then throw some files my way!</p>
				{/if}
			</div>
		{:else}
			<!-- attention: these markers have custom css at the bottom of thise file -->
			<div class="no-uncommitted text-color-3 rounded py-6 text-center font-mono" data-dnd-ignore>
				No uncommitted changes on this branch
			</div>
		{/if}
	{/if}
</div>

<style lang="postcss">
	.wrapper {
		display: flex;
		flex-direction: column;
		flex-shrink: 1;
		overflow: hidden;
	}
	.header {
		color: var(----clr-theme-scale-ntrl-0);
		display: flex;
		padding-top: var(--space-12);
		padding-bottom: var(--space-12);
		padding-left: var(--space-16);
		padding-right: var(--space-12);
		justify-content: space-between;
		border-color: var(--clr-theme-container-outline-light);
	}
	.scrollbar {
		position: relative;
		display: flex;
		overflow: hidden;
	}
	.files {
		display: flex;
		flex-direction: column;
		flex-grow: 1;
		gap: var(--space-4);
		padding-top: var(--space-8);
		padding-bottom: var(--space-16);
		padding-left: var(--space-12);
		padding-right: var(--space-12);
		overflow-y: scroll;
		overflow-x: hidden;
		overscroll-behavior: none;
	}
	.no-uncommitted {
		flex-grow: 1;
	}
	.new-branch {
		flex-grow: 1;
	}
</style>
