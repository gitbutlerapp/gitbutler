<script lang="ts">
	import Button from './Button.svelte';
	import HunkContextMenu from './HunkContextMenu.svelte';
	import HunkLine from './HunkLine.svelte';
	import Scrollbar from './Scrollbar.svelte';
	import { Project } from '$lib/backend/projects';
	import { draggable } from '$lib/dragging/draggable';
	import { draggableHunk } from '$lib/dragging/draggables';
	import { getContextByClass } from '$lib/utils/context';
	import { getContext, onDestroy } from 'svelte';
	import { SETTINGS_CONTEXT, type SettingsStore } from '$lib/settings/userSettings';
	import type { HunkSection } from '$lib/utils/fileSections';
	import type { Ownership } from '$lib/vbranches/ownership';
	import type { Hunk } from '$lib/vbranches/types';
	import type { Writable } from 'svelte/store';

	export let viewport: HTMLDivElement | undefined = undefined;
	export let contents: HTMLDivElement | undefined = undefined;
	export let filePath: string;
	export let section: HunkSection;
	export let branchId: string | undefined;
	export let minWidth: number;
	export let selectable = false;
	export let isUnapplied: boolean;
	export let isFileLocked: boolean;
	export let readonly: boolean = false;
	export let selectedOwnership: Writable<Ownership> | undefined = undefined;
	export let linesModified: number;

	const userSettings = getContext(SETTINGS_CONTEXT) as SettingsStore;
	const project = getContextByClass(Project);

	function onHunkSelected(hunk: Hunk, isSelected: boolean) {
		if (!selectedOwnership) return;
		if (isSelected) {
			selectedOwnership.update((ownership) => ownership.addHunk(hunk.filePath, hunk.id));
		} else {
			selectedOwnership.update((ownership) => ownership.removeHunk(hunk.filePath, hunk.id));
		}
	}

	function updateContextMenu(filePath: string) {
		if (popupMenu) popupMenu.$destroy();
		return new HunkContextMenu({
			target: document.body,
			props: { projectPath: project.path, filePath }
		});
	}

	$: popupMenu = updateContextMenu(filePath);

	$: draggingDisabled = readonly || isUnapplied || section.hunk.locked || !branchId;

	onDestroy(() => {
		if (popupMenu) {
			popupMenu.$destroy();
		}
	});

	let alwaysShow = false;
</script>

<div class="scrollable">
	<div
		bind:this={viewport}
		tabindex="0"
		role="cell"
		use:draggable={{
			...draggableHunk(branchId, section.hunk),
			disabled: draggingDisabled
		}}
		on:contextmenu|preventDefault
		class="hunk hide-native-scrollbar"
		class:readonly
		class:opacity-60={section.hunk.locked && !isFileLocked}
	>
		<div bind:this={contents} class="hunk__bg-stretch">
			{#if linesModified > 1000 && !alwaysShow}
				<div class="flex flex-col p-1">
					Change hidden as large diffs may slow down the UI
					<Button kind="outlined" color="neutral" on:click={() => (alwaysShow = true)}
						>show anyways</Button
					>
				</div>
			{:else}
				{#each section.subSections as subsection}
					{@const hunk = section.hunk}
					{#each subsection.lines.slice(0, subsection.expanded ? subsection.lines.length : 0) as line}
						<HunkLine
							{line}
							{filePath}
							{readonly}
							{minWidth}
							{selectable}
							{draggingDisabled}
							tabSize={$userSettings.tabSize}
							selected={$selectedOwnership?.containsHunk(hunk.filePath, hunk.id)}
							on:selected={(e) => onHunkSelected(hunk, e.detail)}
							sectionType={subsection.sectionType}
							on:contextmenu={(e) =>
								popupMenu.openByMouse(e, {
									hunk,
									section: subsection,
									lineNumber: line.afterLineNumber ? line.afterLineNumber : line.beforeLineNumber
								})}
						/>
					{/each}
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

		background: var(--clr-theme-container-light);
		border-radius: var(--radius-s);
		border: 1px solid var(--clr-theme-container-outline-light);
		transition: border-color var(--transition-fast);
	}

	.hunk__bg-stretch {
		width: 100%;
		min-width: max-content;
	}
</style>
