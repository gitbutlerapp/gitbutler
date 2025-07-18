<script lang="ts">
	import HunkContextMenu from '$components/HunkContextMenu.svelte';
	import HunkDiff from '$components/HunkDiff.svelte';
	import LargeDiffMessage from '$components/LargeDiffMessage.svelte';
	import { BranchStack } from '$lib/branches/branch';
	import { SelectedOwnership } from '$lib/branches/ownership';
	import { draggableChips } from '$lib/dragging/draggable';
	import { HunkDropData } from '$lib/dragging/draggables';
	import { DropzoneRegistry } from '$lib/dragging/registry';
	import { type Hunk } from '$lib/hunks/hunk';
	import { SETTINGS, type Settings } from '$lib/settings/userSettings';
	import { type HunkSection } from '$lib/utils/fileSections';
	import {
		getContext,
		getContextStoreBySymbol,
		maybeGetContextStore
	} from '@gitbutler/shared/context';
	import type { Writable } from 'svelte/store';

	interface Props {
		projectId: string;
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

	const {
		projectId,
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
	const stack = maybeGetContextStore(BranchStack);
	const dropzoneRegistry = getContext(DropzoneRegistry);

	let alwaysShow = $state(false);
	let viewport = $state<HTMLDivElement>();
	let contextMenu = $state<ReturnType<typeof HunkContextMenu>>();
	const draggingDisabled = $derived(isUnapplied || readonly);

	function onHunkSelected(hunk: Hunk, isSelected: boolean) {
		if (!selectedOwnership) return;
		if (isSelected) {
			selectedOwnership.update((ownership) => ownership.select(hunk.filePath, hunk));
		} else {
			selectedOwnership.update((ownership) => ownership.ignore(hunk.filePath, hunk.id));
		}
	}
</script>

<HunkContextMenu bind:this={contextMenu} trigger={viewport} {projectId} {filePath} {readonly} />

<div class="scrollable">
	<div
		tabindex="0"
		role="cell"
		bind:this={viewport}
		class="hunk"
		class:opacity-60={section.hunk.locked && !isFileLocked}
		oncontextmenu={(e) => e.preventDefault()}
		use:draggableChips={{
			label: section.hunk.diff.split('\n')[0],
			data: new HunkDropData($stack?.id || '', section.hunk, section.hunk.lockedTo, commitId),
			disabled: draggingDisabled,
			chipType: 'hunk',
			dropzoneRegistry
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
				wrapText={$userSettings.wrapText}
				diffFont={$userSettings.diffFont}
				diffLigatures={$userSettings.diffLigatures}
				diffContrast={$userSettings.diffContrast}
				inlineUnifiedDiffs={$userSettings.inlineUnifiedDiffs}
				hunk={section.hunk}
				onclick={() => {
					contextMenu?.close();
				}}
				subsections={section.subSections}
				handleSelected={(hunk, isSelected) => onHunkSelected(hunk, isSelected)}
				handleLineContextMenu={({ event, beforeLineNumber, afterLineNumber, hunk, subsection }) => {
					contextMenu?.open(event, {
						hunk,
						section: subsection,
						beforeLineNumber,
						afterLineNumber
					});
				}}
			/>
		{/if}
	</div>
</div>

<style lang="postcss">
	.scrollable {
		display: flex;
		position: relative;
		flex-direction: column;
	}

	.hunk {
		width: 100%;
		overflow-x: auto;
		will-change: transform;
		user-select: text;
	}
</style>
