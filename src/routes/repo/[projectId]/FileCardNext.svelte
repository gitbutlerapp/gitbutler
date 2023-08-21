<script lang="ts">
	import { ContentSection, HunkSection, parseFileSections } from './fileSections';
	import { createEventDispatcher } from 'svelte';
	import type { File } from '$lib/vbranches/types';
	import RenderedLine from './RenderedLine.svelte';
	import {
		IconTriangleUp,
		IconTriangleDown,
		IconExpandUpDown,
		IconExpandUp,
		IconExpandDown
	} from '$lib/icons';
	import type { BranchController } from '$lib/vbranches/branchController';
	import { getContext } from 'svelte';
	import { dzTrigger } from './dropZone';
	import IconExpandUpDownSlim from '$lib/icons/IconExpandUpDownSlim.svelte';
	import { getVSIFileIcon } from '$lib/ext-icons';
	import { slide } from 'svelte/transition';
	import { SETTINGS_CONTEXT, type SettingsStore } from '$lib/userSettings';
	import { summarizeHunk } from '$lib/summaries';
	import Tooltip from '$lib/components/Tooltip/Tooltip.svelte';
	import IconLock from '$lib/icons/IconLock.svelte';
	import HunkContextMenu from './HunkContextMenu.svelte';

	export let file: File;
	export let conflicted: boolean;
	export let projectId: string;
	export let dzType: string;
	export let projectPath: string;
	export let expanded: boolean | undefined;
	export let branchController: BranchController;

	const userSettings = getContext<SettingsStore>(SETTINGS_CONTEXT);
	const dispatch = createEventDispatcher<{
		expanded: boolean;
	}>();

	let popupMenu = new HunkContextMenu({
		target: document.body,
		props: { projectPath, file }
	});

	function boldenFilename(filepath: string): string {
		const parts = filepath.split('/');
		if (parts.length == 0) return '';
		return (
			'<span class="font-bold text-light-800 dark:text-dark-50 mr-1">' +
			parts[parts.length - 1] +
			'</span>/' +
			parts.slice(0, -1).join('/')
		);
	}

	function parseFile(file: File) {
		// When we toggle expansion status on sections we need to assign
		// `sections = sections` to redraw, and why we do not use a reactive
		// variable.
		sections = parseFileSections(file);
	}
	$: parseFile(file);

	let sections: (HunkSection | ContentSection)[] = [];
	$: maxLineNumber = sections[sections.length - 1]?.maxLineNumber;

	function getGutterMinWidth(max: number) {
		if (max >= 1000) return 2;
		if (max >= 100) return 1.5;
		if (max >= 10) return 1.25;
		return 1;
	}

	$: minWidth = getGutterMinWidth(maxLineNumber);

	function getAllHunksOwnership(): string {
		return file.id + ':' + file.hunks.map((h) => h.id).join(',');
	}

	$: isFileLocked = sections
		.filter((section): section is HunkSection => section instanceof HunkSection)
		.some((section) => section.hunk.locked);
</script>

<div
	draggable={!isFileLocked}
	use:dzTrigger={{ type: dzType }}
	on:dragstart={(e) => e.dataTransfer?.setData('text/hunk', getAllHunksOwnership())}
	role="group"
	class="changed-file inner"
	class:opacity-80={isFileLocked}
>
	<div
		class="flex w-full flex-col justify-center gap-2 border-b border-t border-light-300 bg-light-50 py-1 text-light-900 dark:border-dark-500 dark:bg-dark-800 dark:text-light-300"
	>
		<div
			class="flex cursor-default pl-2"
			role="button"
			tabindex="0"
			on:dblclick|stopPropagation={() => {
				expanded = !expanded;
				dispatch('expanded', expanded);
			}}
		>
			<div
				class="flex-grow overflow-hidden text-ellipsis whitespace-nowrap text-light-800 dark:text-dark-100"
				title={file.path}
			>
				<img
					src={getVSIFileIcon(file.path)}
					alt="js"
					width="13"
					style="width: 0.8125rem"
					class="mr-1 inline"
				/>

				{@html boldenFilename(file.path)}
			</div>
			{#if isFileLocked}
				<div class="flex flex-grow-0">
					<Tooltip
						label="File changes cannot be moved because part of this file was already committed into this branch"
					>
						<IconLock class="h-4 w-4 text-yellow-600" />
					</Tooltip>
				</div>
			{/if}
			<div
				on:click|stopPropagation={() => {
					expanded = !expanded;
					dispatch('expanded', expanded);
				}}
				on:keypress={() => (expanded = !expanded)}
				role="button"
				tabindex="0"
				class="flex-grow-0 cursor-pointer px-3 py-2 text-light-600 dark:text-dark-200"
			>
				{#if !file.binary}
					{#if expanded}
						<IconTriangleUp />
					{:else}
						<IconTriangleDown />
					{/if}
				{/if}
			</div>
		</div>

		{#if conflicted}
			<div class="mb-2 bg-red-500 px-2 py-0 font-bold text-white">
				<button
					class="font-bold text-white"
					on:click={() => branchController.markResolved(projectId, file.path)}
				>
					Mark resolved
				</button>
			</div>
		{/if}

		{#if expanded}
			<div
				class="hunk-change-container flex flex-col rounded px-2"
				transition:slide={{ duration: 150 }}
			>
				{#each sections as section}
					{#if 'hunk' in section}
						{#if $userSettings.aiSummariesEnabled}
							{#await summarizeHunk(section.hunk.diff) then description}
								<div
									class="truncate whitespace-normal pb-1 pl-1 pt-2 text-light-700 dark:text-dark-200"
								>
									{description}
								</div>
							{/await}
						{/if}
						<div
							class="my-1 flex w-full flex-col overflow-hidden rounded border border-light-400 bg-white dark:border-dark-400 dark:bg-dark-900"
						>
							<div
								draggable={!section.hunk.locked}
								tabindex="0"
								role="cell"
								use:dzTrigger={{ type: dzType }}
								on:dragstart={(e) => {
									if ('hunk' in section)
										e.dataTransfer?.setData('text/hunk', file.id + ':' + section.hunk.id);
								}}
								on:dblclick
								class="changed-hunk"
								class:opacity-60={section.hunk.locked && !isFileLocked}
							>
								<div class="w-full overflow-hidden bg-white dark:bg-dark-900">
									{#each section.subSections as subsection, sidx}
										{#each subsection.lines.slice(0, subsection.expanded ? subsection.lines.length : 0) as line}
											<RenderedLine
												{line}
												{minWidth}
												sectionType={subsection.sectionType}
												filePath={file.path}
												on:contextmenu={(e) =>
													popupMenu.openByMouse(e, {
														section: subsection,
														lineNumber: line.afterLineNumber
															? line.afterLineNumber
															: line.beforeLineNumber
													})}
											/>
										{/each}
										{#if !subsection.expanded}
											<div
												class="flex w-full border-light-200 dark:border-dark-400"
												class:border-t={sidx == section.subSections.length - 1 ||
													(sidx > 0 && sidx < section.subSections.length - 1)}
												class:border-b={sidx == 0 ||
													(sidx > 0 && sidx < section.subSections.length - 1)}
											>
												<div
													class="border-r border-light-200 bg-light-25 text-center text-light-500 hover:bg-light-700 hover:text-white dark:border-dark-400 dark:bg-dark-500 dark:text-light-600 dark:hover:bg-dark-400 dark:hover:text-black"
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
												<div class="flex-grow bg-white dark:bg-dark-900" />
											</div>
										{/if}
									{/each}
								</div>
							</div>
						</div>
					{:else}
						{#if section.expanded}
							<div
								class="my-1 flex w-full flex-col overflow-hidden rounded border border-light-200 bg-white dark:border-dark-400 dark:bg-dark-900"
								role="group"
								on:dblclick
							>
								{#each section.lines.slice(0, section.expanded ? section.lines.length : 0) as line}
									<RenderedLine
										{line}
										{minWidth}
										sectionType={section.sectionType}
										filePath={file.path}
										on:contextmenu={(e) =>
											popupMenu.openByMouse(e, {
												section: section,
												lineNumber: line.afterLineNumber
											})}
									/>
								{/each}
							</div>
						{/if}
						{#if !section.expanded}
							<div style:width={`calc(${2 * minWidth}rem - 1px)`} class="flex justify-center">
								<button
									class="px-2 py-1.5 text-sm text-light-600 hover:text-light-700 dark:text-dark-200 dark:hover:text-dark-100"
									on:click={() => {
										if ('expanded' in section) {
											section.expanded = true;
										}
									}}
								>
									<IconExpandUpDownSlim />
								</button>
							</div>
						{/if}
					{/if}
				{/each}
			</div>
		{/if}
	</div>
</div>
