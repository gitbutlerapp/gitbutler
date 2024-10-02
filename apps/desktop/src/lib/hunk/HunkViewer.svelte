<script lang="ts">
	import HunkDiff from './HunkDiff.svelte';
	import { Project } from '$lib/backend/projects';
	import { draggableElement } from '$lib/dragging/draggable';
	import { DraggableHunk } from '$lib/dragging/draggables';
	import HunkContextMenu from '$lib/hunk/HunkContextMenu.svelte';
	import { SETTINGS, type Settings } from '$lib/settings/userSettings';
	import LargeDiffMessage from '$lib/shared/LargeDiffMessage.svelte';
	import { getContext, getContextStoreBySymbol, maybeGetContextStore } from '$lib/utils/context';
	import { type HunkSection } from '$lib/utils/fileSections';
	import { SelectedOwnership } from '$lib/vbranches/ownership';
	import { VirtualBranch, type Hunk } from '$lib/vbranches/types';
	import type { Writable } from 'svelte/store';

	interface Props {
		filePath: string;
		section: HunkSection;
		selectable: boolean;
		isUnapplied: boolean;
		isFileLocked: boolean;
		readonly: boolean;
		minWidth: number;
		linesModified: number;
		commitId?: string | undefined;
	}

	let {
		filePath,
		section,
		linesModified,
		selectable = false,
		isUnapplied,
		isFileLocked,
		minWidth,
		commitId,
		readonly = false
	}: Props = $props();

	const selectedOwnership: Writable<SelectedOwnership> | undefined =
		maybeGetContextStore(SelectedOwnership);
	const userSettings = getContextStoreBySymbol<Settings>(SETTINGS);
	const branch = maybeGetContextStore(VirtualBranch);
	const project = getContext(Project);

	let alwaysShow = $state(false);
	let viewport = $state<HTMLDivElement>();
	let contextMenu = $state<ReturnType<typeof HunkContextMenu>>();
	const draggingDisabled = $derived(isUnapplied);

	function onHunkSelected(hunk: Hunk, isSelected: boolean) {
		if (!selectedOwnership) return;
		if (isSelected) {
			selectedOwnership.update((ownership) => ownership.select(hunk.filePath, hunk));
		} else {
			selectedOwnership.update((ownership) => ownership.ignore(hunk.filePath, hunk.id));
		}
	}
</script>

<HunkContextMenu
	bind:this={contextMenu}
	target={viewport}
	projectPath={project.vscodePath}
	{filePath}
	{readonly}
/>

<div class="scrollable">
	<div
		tabindex="0"
		role="cell"
		bind:this={viewport}
		class="hunk"
		class:opacity-60={section.hunk.locked && !isFileLocked}
		oncontextmenu={(e) => e.preventDefault()}
		use:draggableElement={{
			data: new DraggableHunk($branch?.id || '', section.hunk, section.hunk.lockedTo, commitId),
			disabled: draggingDisabled
		}}
	>
		{#if linesModified > 2500 && !alwaysShow}
			<LargeDiffMessage
				handleShow={() => {
					alwaysShow = true;
				}}
			/>
		{:else}
			<HunkDiff
				{readonly}
				{filePath}
				{minWidth}
				{selectable}
				{draggingDisabled}
				tabSize={$userSettings.tabSize}
				diffFont={$userSettings.diffFont}
				diffLigatures={$userSettings.diffLigatures}
				inlineUnifiedDiffs={$userSettings.inlineUnifiedDiffs}
				hunk={section.hunk}
				onclick={() => {
					contextMenu?.close();
				}}
				subsections={section.subSections}
				handleSelected={(hunk, isSelected) => onHunkSelected(hunk, isSelected)}
				handleLineContextMenu={({ event, lineNumber, hunk, subsection }) => {
					contextMenu?.open(event, {
						hunk,
						section: subsection,
						lineNumber: lineNumber
					});
				}}
			/>
		{/if}
	</div>
</div>

<style lang="postcss">
	.scrollable {
		display: flex;
		flex-direction: column;
		position: relative;
	}

	.hunk {
		width: 100%;
		user-select: text;
		overflow-x: auto;
		will-change: transform;
	}
</style>
