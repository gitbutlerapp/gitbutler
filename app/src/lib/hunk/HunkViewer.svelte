<script lang="ts">
	import { Project } from '$lib/backend/projects';
	// import { draggableElement } from '$lib/dragging/draggable';
	// import { DraggableHunk } from '$lib/dragging/draggables';
	import HunkContextMenu from '$lib/hunk/HunkContextMenu.svelte';
	// import HunkLines from '$lib/hunk/HunkLines.svelte';
	import { SETTINGS, type Settings } from '$lib/settings/userSettings';
	import LargeDiffMessage from '$lib/shared/LargeDiffMessage.svelte';
	// import Scrollbar from '$lib/shared/Scrollbar.svelte';
	import { create } from '$lib/utils/codeHighlight';
	import { getContext, getContextStoreBySymbol, maybeGetContextStore } from '$lib/utils/context';
	import { SectionType, type Line } from '$lib/utils/fileSections';
	import { type HunkSection, type ContentSection } from '$lib/utils/fileSections';
	import { Ownership } from '$lib/vbranches/ownership';
	import { VirtualBranch, type Hunk } from '$lib/vbranches/types';
	import { diff_match_patch } from 'diff-match-patch';
	import type { Writable } from 'svelte/store';
	import HunkDiff from './HunkDiff.svelte';
	// import ListItem from '$lib/shared/ListItem.svelte';

	interface Props {
		filePath: string;
		section: HunkSection;
		minWidth: number;
		selectable: boolean;
		isUnapplied: boolean;
		isFileLocked: boolean;
		readonly: boolean;
		linesModified: number;
	}

	let {
		filePath,
		section,
		linesModified,
		minWidth,
		selectable = false,
		isUnapplied,
		isFileLocked,
		readonly = false
	}: Props = $props();

	$inspect('section', section);

	const selectedOwnership: Writable<Ownership> | undefined = maybeGetContextStore(Ownership);
	const userSettings = getContextStoreBySymbol<Settings>(SETTINGS);
	const branch = maybeGetContextStore(VirtualBranch);
	const project = getContext(Project);

	let alwaysShow = $state(false);
	let contents = $state<HTMLDivElement>();
	let viewport = $state<HTMLDivElement>();
	let contextMenu = $state<HunkContextMenu>();
	const draggingDisabled = $derived(readonly || isUnapplied);

	function onHunkSelected(hunk: Hunk, isSelected: boolean) {
		if (!selectedOwnership) return;
		if (isSelected) {
			selectedOwnership.update((ownership) => ownership.add(hunk.filePath, hunk));
		} else {
			selectedOwnership.update((ownership) => ownership.remove(hunk.filePath, hunk.id));
		}
	}
</script>

<div class="scrollable">
	<div tabindex="0" role="cell">
		<div class="hunk__bg-stretch">
			{#if linesModified > 2500 && !alwaysShow}
				<LargeDiffMessage
					on:show={() => {
						alwaysShow = true;
					}}
				/>
			{:else}
				<HunkDiff hunk={section.hunk} {filePath} subsections={section.subSections} />
			{/if}
		</div>
	</div>
</div>

<style lang="postcss">
	.scrollable {
		display: flex;
		flex-direction: column;
		position: relative;
		border-radius: var(--radius-s);
		overflow-x: scroll;

		& > div {
			width: 100%;
		}
	}

	.hunk {
		display: flex;
		flex-direction: column;
		overflow-x: auto;
		user-select: text;

		background: var(--clr-bg-1);
		border-radius: var(--radius-s);
		border: 1px solid var(--clr-border-2);
		transition: border-color var(--transition-fast);
	}

	.hunk__bg-stretch {
		width: 100%;
		min-width: max-content;
	}
</style>
