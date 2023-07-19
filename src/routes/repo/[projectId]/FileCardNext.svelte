<script lang="ts">
	import { ContentSection, HunkSection, parseFileSections } from './fileSections';
	import { createEventDispatcher } from 'svelte';
	import { open } from '@tauri-apps/api/shell';
	import type { File } from '$lib/vbranches';
	import RenderedLine from './RenderedLine.svelte';
	import {
		IconTriangleUp,
		IconTriangleDown,
		IconExpandUpDown,
		IconExpandUp,
		IconExpandDown
	} from '$lib/icons';
	import type { BranchController, Hunk } from '$lib/vbranches';
	import { BRANCH_CONTROLLER_KEY } from '$lib/vbranches/branchController';
	import { getContext } from 'svelte';
	import { dzTrigger } from './dropZone';
	import IconExpandUpDownSlim from '$lib/icons/IconExpandUpDownSlim.svelte';
	import PopupMenu from '$lib/components/PopupMenu/PopupMenu.svelte';
	import PopupMenuItem from '$lib/components/PopupMenu/PopupMenuItem.svelte';

	export let file: File;
	export let conflicted: boolean;
	export let projectId: string;
	export let dzType: string;
	export let maximized: boolean;
	export let projectPath: string;
	export let expanded: boolean | undefined;

	const branchController = getContext<BranchController>(BRANCH_CONTROLLER_KEY);
	const dispatch = createEventDispatcher<{
		expanded: boolean;
	}>();

	let popupMenu: PopupMenu;

	function boldenFilename(filepath: string): string {
		const parts = filepath.split('/');
		if (parts.length == 0) return '';
		return (
			parts.slice(0, -1).join('/') +
			'/<span class="font-bold text-light-800 dark:text-dark-50">' +
			parts[parts.length - 1] +
			'</span>'
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
		if (max >= 1000) return 2.25;
		if (max >= 100) return 1.75;
		if (max >= 10) return 1.5;
		return 1.25;
	}

	$: minWidth = getGutterMinWidth(maxLineNumber);

	function getAllHunksOwnership(): string {
		return file.id + ':' + file.hunks.map((h) => h.id).join(',');
	}
</script>

<div
	draggable="true"
	use:dzTrigger={{ type: dzType }}
	on:dragstart={(e) => e.dataTransfer?.setData('text/hunk', getAllHunksOwnership())}
	role="group"
	class="changed-file inner"
>
	<div
		class="flex w-full flex-col justify-center gap-2 border-b border-t border-light-400 bg-light-50 py-1 text-light-900 dark:border-dark-400 dark:bg-dark-700 dark:text-light-300"
	>
		<div class="flex pl-2">
			<div
				class="flex-grow overflow-hidden text-ellipsis whitespace-nowrap text-light-800 dark:text-dark-100"
				title={file.path}
			>
				{@html boldenFilename(file.path)}
			</div>
			<div
				on:click={() => {
					expanded = !expanded;
					dispatch('expanded', expanded);
				}}
				on:keypress={() => (expanded = !expanded)}
				role="button"
				tabindex="0"
				class="cursor-pointer p-2 text-light-600 dark:text-dark-200"
			>
				{#if expanded}
					<IconTriangleUp />
				{:else}
					<IconTriangleDown />
				{/if}
			</div>
		</div>

		{#if conflicted}
			<div class="mx-2 rounded bg-red-700 p-2 text-white">
				<div>Conflicted</div>
				<button on:click={() => branchController.markResolved(projectId, file.path)}>Resolve</button
				>
			</div>
		{/if}

		{#if expanded}
			<div class="hunk-change-container flex flex-col rounded px-2">
				{#each sections as section, idx}
					{#if 'hunk' in section}
						<div
							class="my-1 flex w-full flex-col overflow-hidden rounded border border-light-400 bg-white dark:border-dark-400 dark:bg-dark-900"
						>
							<div
								draggable="true"
								tabindex="0"
								role="cell"
								use:dzTrigger={{ type: dzType }}
								on:dragstart={(e) => {
									if ('hunk' in section)
										e.dataTransfer?.setData('text/hunk', file.id + ':' + section.hunk.id);
								}}
								on:dblclick
								class="changed-hunk"
							>
								<div class="w-full overflow-hidden bg-white dark:bg-dark-900">
									{#each section.subSections as subsection, sidx}
										{#each subsection.lines.slice(0, subsection.expanded ? subsection.lines.length : 0) as line}
											<RenderedLine
												{line}
												{minWidth}
												{maximized}
												sectionType={subsection.sectionType}
												filePath={file.path}
												on:contextmenu={(e) =>
													popupMenu.openByMouse(e, {
														section: subsection,
														lineNumber: line.afterLineNumber
													})}
											/>
										{/each}
										{#if !subsection.expanded}
											<div
												class="flex h-5 w-full border-light-200 dark:border-dark-400"
												class:border-t={sidx == section.subSections.length - 1 ||
													(sidx > 0 && sidx < section.subSections.length - 1)}
												class:border-b={sidx == 0 ||
													(sidx > 0 && sidx < section.subSections.length - 1)}
											>
												<div
													class="border-r border-light-200 bg-light-25 text-center text-light-500 hover:bg-light-700 hover:text-white dark:border-dark-400 dark:bg-dark-500 dark:text-white"
													style:min-width={`${2 * minWidth}rem`}
												>
													<button
														class="flex justify-center py-0.5 text-sm dark:text-dark-200 dark:hover:text-dark-100"
														style:width={`${2 * minWidth}rem`}
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
												<div class="flex-grow bg-white dark:bg-dark-600" />
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
										{maximized}
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
							<div style:width={`${2 * minWidth}rem`} class="flex justify-center">
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
	<PopupMenu bind:this={popupMenu} let:item>
		<PopupMenuItem
			on:click={() => {
				if ('expanded' in item.section) {
					item.section.expanded = false;
					sections = sections;
				}
			}}
		>
			Collapse
		</PopupMenuItem>
		<PopupMenuItem
			on:click={() => {
				console.log(item);
				const url = `vscode://file${projectPath}/${file.path}:${item.lineNumber}`;
				console.log(url);
				open(url);
			}}
		>
			Open in Visual Studio Code
		</PopupMenuItem>
	</PopupMenu>
</div>
