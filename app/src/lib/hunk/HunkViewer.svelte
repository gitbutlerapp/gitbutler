<script lang="ts">
	import { Project } from '$lib/backend/projects';
	import { draggableElement } from '$lib/dragging/draggable';
	import { DraggableHunk } from '$lib/dragging/draggables';
	import HunkContextMenu from '$lib/hunk/HunkContextMenu.svelte';
	import HunkLines from '$lib/hunk/HunkLines.svelte';
	import { SETTINGS, type Settings } from '$lib/settings/userSettings';
	import LargeDiffMessage from '$lib/shared/LargeDiffMessage.svelte';
	import Scrollbar from '$lib/shared/Scrollbar.svelte';
	import { getContext, getContextStoreBySymbol, maybeGetContextStore } from '$lib/utils/context';
	import { Ownership } from '$lib/vbranches/ownership';
	import { VirtualBranch, type Hunk } from '$lib/vbranches/types';
	import type { HunkSection } from '$lib/utils/fileSections';
	import type { Writable } from 'svelte/store';

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
		minWidth,
		selectable = false,
		isUnapplied,
		isFileLocked,
		readonly = false,
		linesModified
	}: Props = $props();

	const selectedOwnership: Writable<Ownership> | undefined = maybeGetContextStore(Ownership);
	const userSettings = getContextStoreBySymbol<Settings>(SETTINGS);
	const branch = maybeGetContextStore(VirtualBranch);
	const project = getContext(Project);

	let contents = $state<HTMLDivElement>();
	let viewport = $state<HTMLDivElement>();
	let contextMenu = $state<HunkContextMenu>();
	let alwaysShow = $state(false);
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

<HunkContextMenu
	bind:this={contextMenu}
	target={viewport}
	projectPath={project.vscodePath}
	{filePath}
	{readonly}
/>

<div class="scrollable">
	<div
		bind:this={viewport}
		tabindex="0"
		role="cell"
		use:draggableElement={{
			data: new DraggableHunk($branch?.id || '', section.hunk),
			disabled: draggingDisabled
		}}
		oncontextmenu={(e) => e.preventDefault()}
		class="hunk hide-native-scrollbar"
		class:readonly
		class:opacity-60={section.hunk.locked && !isFileLocked}
	>
		<div bind:this={contents} class="hunk__bg-stretch">
			{#if linesModified > 2500 && !alwaysShow}
				<LargeDiffMessage
					on:show={() => {
						alwaysShow = true;
					}}
				/>
			{:else}
				{#each section.subSections as subsection}
					{@const hunk = section.hunk}
					<HunkLines
						lines={subsection.lines}
						{filePath}
						{readonly}
						{minWidth}
						{selectable}
						{draggingDisabled}
						tabSize={$userSettings.tabSize}
						selected={$selectedOwnership?.contains(hunk.filePath, hunk.id) ?? false}
						sectionType={subsection.sectionType}
						handleSelected={(isSelected) => onHunkSelected(hunk, isSelected)}
						handleClick={() => {
							contextMenu?.close();
						}}
						handleLineContextMenu={({ event, lineNumber }) => {
							contextMenu?.open(event, {
								hunk,
								section: subsection,
								lineNumber: lineNumber
							});
						}}
					/>
				{/each}
			{/if}
		</div>
	</div>
	<Scrollbar {viewport} {contents} horz />
</div>

<style lang="postcss">
	.scrollable {
		display: flex;
		flex-direction: column;
		position: relative;
		border-radius: var(--radius-s);
		overflow: hidden;
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
