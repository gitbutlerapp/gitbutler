<script lang="ts">
	import { ContentSection, HunkSection, parseFileSections } from './fileSections';
	import { onDestroy } from 'svelte';
	import type { File, Hunk } from '$lib/vbranches/types';
	import { draggable } from '$lib/utils/draggable';
	import type { Ownership } from '$lib/vbranches/ownership';
	import type { Writable } from 'svelte/store';
	import RenderedLine from './RenderedLine.svelte';
	import { IconExpandUpDown, IconExpandUp, IconExpandDown } from '$lib/icons';
	import type { BranchController } from '$lib/vbranches/branchController';
	import { getContext } from 'svelte';
	import { slide } from 'svelte/transition';
	import { quintOut } from 'svelte/easing';
	import { SETTINGS_CONTEXT, type SettingsStore } from '$lib/settings/userSettings';
	import { summarizeHunk } from './summaries';
	import HunkContextMenu from './HunkContextMenu.svelte';
	import { draggableFile, draggableHunk } from '$lib/draggables';
	import FileCardHeader from './FileCardHeader.svelte';
	import Resizer from '$lib/components/Resizer.svelte';
	import lscache from 'lscache';
	import { persisted } from '$lib/persisted/persisted';
	import ScrollableContainer from '$lib/components/ScrollableContainer.svelte';

	export let projectId: string;
	export let branchId: string;
	export let file: File;
	export let conflicted: boolean;
	export let projectPath: string | undefined;
	export let branchController: BranchController;
	export let readonly = false;
	export let selectable = false;
	export let selectedOwnership: Writable<Ownership>;

	let rsViewport: HTMLElement;

	const defaultFileWidthRem = persisted<number | undefined>(30, 'defaulFileWidth' + projectId);
	const fileWidthKey = 'fileWidth_';
	let fileWidth: number;

	const userSettings = getContext<SettingsStore>(SETTINGS_CONTEXT);

	function updateContextMenu(file: File) {
		if (popupMenu) popupMenu.$destroy();
		return new HunkContextMenu({
			target: document.body,
			props: { projectPath, file, branchController }
		});
	}

	$: popupMenu = updateContextMenu(file);

	let sections: (HunkSection | ContentSection)[] = [];

	function parseFile(file: File) {
		// When we toggle expansion status on sections we need to assign
		// `sections = sections` to redraw, and why we do not use a reactive
		// variable.
		if (!file.binary && !file.large) sections = parseFileSections(file);
	}
	$: parseFile(file);

	$: maxLineNumber = sections[sections.length - 1]?.maxLineNumber;

	function getGutterMinWidth(max: number) {
		if (max >= 1000) return 2;
		if (max >= 100) return 1.5;
		if (max >= 10) return 1.25;
		return 1;
	}

	$: minWidth = getGutterMinWidth(maxLineNumber);

	$: isFileLocked = sections
		.filter((section): section is HunkSection => section instanceof HunkSection)
		.some((section) => section.hunk.locked);

	function onHunkSelected(hunk: Hunk, isSelected: boolean) {
		if (isSelected) {
			selectedOwnership.update((ownership) => ownership.addHunk(hunk.filePath, hunk.id));
		} else {
			selectedOwnership.update((ownership) => ownership.removeHunk(hunk.filePath, hunk.id));
		}
	}

	onDestroy(() => {
		if (popupMenu) {
			popupMenu.$destroy();
		}
	});

	function computedAddedRemoved(section: HunkSection | ContentSection): {
		added: any;
		removed: any;
	} {
		if (section instanceof HunkSection) {
			const lines = section.hunk.diff.split('\n');
			return {
				added: lines.filter((l) => l.startsWith('+')).length,
				removed: lines.filter((l) => l.startsWith('-')).length
			};
		}
		return {
			added: 0,
			removed: 0
		};
	}

	fileWidth = lscache.get(fileWidthKey + file.id);
</script>

<div
	class="resize-viewport"
	bind:this={rsViewport}
	transition:slide={{ duration: 170, easing: quintOut, axis: 'x' }}
	use:draggable={{
		...draggableFile(branchId, file),
		disabled: readonly
	}}
>
	<div
		id={`file-${file.id}`}
		class="file-card"
		style:width={`${fileWidth || $defaultFileWidthRem}rem`}
		class:opacity-80={isFileLocked}
	>
		<FileCardHeader {file} {isFileLocked} on:close />
		{#if conflicted}
			<div class="mb-2 bg-red-500 px-2 py-0 font-bold text-white">
				<button
					class="font-bold text-white"
					on:click={() => branchController.markResolved(file.path)}
				>
					Mark resolved
				</button>
			</div>
		{/if}

		<ScrollableContainer wide>
			<div class="hunks">
				{#if file.binary}
					Binary content not shown
				{:else if file.large}
					Diff too large to be shown
				{:else}
					{#each sections as section}
						{@const { added, removed } = computedAddedRemoved(section)}
						{#if 'hunk' in section}
							{#if $userSettings.aiSummariesEnabled && !file.binary}
								{#await summarizeHunk(section.hunk.diff) then description}
									<div class="text-color-3 truncate whitespace-normal pb-1 pl-1 pt-2">
										{description}
									</div>
								{/await}
							{/if}
							<div class="hunk-wrapper">
								<div class="stats text-base-11">
									<span class="added">+{added}</span>
									<span class="removed">+{removed}</span>
								</div>
								<div
									tabindex="0"
									role="cell"
									use:draggable={{
										...draggableHunk(branchId, section.hunk),
										disabled: readonly
									}}
									on:dblclick
									class="hunk"
									class:opacity-60={section.hunk.locked && !isFileLocked}
								>
									<div class="hunk__inner custom-scrollbar">
										<div class="hunk__inner_inner">
											{#each section.subSections as subsection, sidx}
												{@const hunk = section.hunk}
												{#each subsection.lines.slice(0, subsection.expanded ? subsection.lines.length : 0) as line}
													<RenderedLine
														{line}
														{minWidth}
														selected={$selectedOwnership.containsHunk(hunk.filePath, hunk.id)}
														on:selected={(e) => onHunkSelected(hunk, e.detail)}
														{selectable}
														sectionType={subsection.sectionType}
														filePath={file.path}
														on:contextmenu={(e) =>
															popupMenu.openByMouse(e, {
																hunk,
																section: subsection,
																lineNumber: line.afterLineNumber
																	? line.afterLineNumber
																	: line.beforeLineNumber
															})}
													/>
												{/each}
												{#if !subsection.expanded}
													<div
														role="group"
														class="border-color-3 flex w-full"
														class:border-t={sidx == section.subSections.length - 1 ||
															(sidx > 0 && sidx < section.subSections.length - 1)}
														class:border-b={sidx == 0 ||
															(sidx > 0 && sidx < section.subSections.length - 1)}
														on:contextmenu|preventDefault={(e) =>
															popupMenu.openByMouse(e, {
																section: section,
																hunk
															})}
													>
														<div
															class="bg-color-4 text-color-4 hover:text-color-2 border-color-3 border-r text-center"
															style:min-width={`calc(${2 * minWidth}rem - 1px)`}
														>
															<button
																class="flex justify-center py-0.5 text-sm"
																style:width={`calc(${2 * minWidth}rem - 1px)`}
																on:click={() => {
																	if ('expanded' in subsection) {
																		subsection.expanded = true;
																	}
																}}
															>
																{#if sidx == 0}
																	<IconExpandUp />
																{:else if sidx == section.subSections.length - 1}
																	<IconExpandDown />
																{:else}
																	<IconExpandUpDown />
																{/if}
															</button>
														</div>
														<div class="bg-color-4 flex-grow" />
													</div>
												{/if}
											{/each}
										</div>
									</div>
								</div>
							</div>
						{/if}
					{/each}
				{/if}
			</div>
		</ScrollableContainer>
	</div>
	<Resizer
		viewport={rsViewport}
		direction="right"
		inside
		minWidth={240}
		on:width={(e) => {
			fileWidth = e.detail / (16 * $userSettings.zoom);
			lscache.set(fileWidthKey + file.id, fileWidth, 7 * 1440); // 7 day ttl
			$defaultFileWidthRem = fileWidth;
		}}
	/>
</div>

<style lang="postcss">
	.file-card {
		background: var(--clr-theme-container-light);
		border-left: 1px solid var(--clr-theme-container-outline-light);
		overflow: hidden;
		display: flex;
		flex-direction: column;
	}
	.hunks {
		display: flex;
		flex-direction: column;
		position: relative;
		max-height: 100%;
		flex-shrink: 0;
		padding: var(--space-16);
		gap: var(--space-16);
	}
	.hunk-wrapper {
		display: flex;
		flex-direction: column;
		gap: var(--space-10);
	}
	.hunk {
		display: flex;
		flex-direction: column;
		overflow-x: hidden;
		&:focus-within {
			& .hunk__inner {
				overflow-x: auto;
				border-color: var(--clr-theme-container-outline-pale);
			}
		}
	}
	.hunk__inner {
		display: flex;
		flex-direction: column;
		overflow: hidden;
		background: var(--clr-theme-container-light);
		border-radius: var(--radius-s);
		border: 1px solid var(--clr-theme-container-outline-light);
		overflow-x: hidden;
		overscroll-behavior: none;
		transition: border-color var(--transition-fast);
	}
	.hunk__inner_inner {
		/* TODO: Rename this class */
		width: 100%;
		min-width: max-content;
		user-select: text !important;
		cursor: grab;
	}
	.added {
		color: #45b156;
	}
	.removed {
		color: #ff3e00;
	}
	.resize-viewport {
		position: relative;
		display: flex;
	}

	@keyframes wiggle {
		0% {
			transform: rotate(0deg);
		}
		40% {
			transform: rotate(0deg);
		}
		60% {
			transform: rotate(2deg);
		}
		80% {
			transform: rotate(-2deg);
		}
		100% {
			transform: rotate(0deg);
		}
	}
	:global(.wiggle) {
		animation: wiggle 0.5s infinite;
	}
</style>
