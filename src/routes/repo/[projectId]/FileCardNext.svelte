<script lang="ts">
	import { parseFileSections } from './fileSections';
	import { createEventDispatcher } from 'svelte';
	import type { File } from '$lib/vbranches';
	import RenderedLine from './RenderedLine.svelte';
	import {
		IconTriangleUp,
		IconTriangleDown,
		IconExpandUpDown,
		IconExpandUp,
		IconExpandDown
	} from '$lib/icons';
	export let file: File;
	import type { BranchController, Hunk } from '$lib/vbranches';
	import { BRANCH_CONTROLLER_KEY } from '$lib/vbranches/branchController';
	import { getContext } from 'svelte';
	const branchController = getContext<BranchController>(BRANCH_CONTROLLER_KEY);
	export let conflicted: boolean;
	export let projectId: string;
	const dispatch = createEventDispatcher<{
		expanded: boolean;
	}>();
	export let expanded: boolean | undefined;
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
	$: sections = parseFileSections(file);
</script>

<div
	class="flex w-full flex-col justify-center gap-2 rounded border border-light-300 bg-light-50 text-light-900 dark:border-dark-400 dark:bg-dark-700 dark:text-light-300"
>
	<div class="flex px-2 pt-2">
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
			<button on:click={() => branchController.markResolved(projectId, file.path)}>Resolve</button>
		</div>
	{/if}

	{#if expanded}
		<div class="hunk-change-container flex flex-col gap-2 rounded px-2 pb-2">
			<div
				class="flex w-full flex-col overflow-hidden rounded border border-light-200 bg-white dark:border-dark-400 dark:bg-dark-900"
			>
				{#each sections as section, idx}
					{#if 'hunk' in section}
						<div class="">
							{#each section.subSections as subsection, sidx}
								<!-- prettier-ignore -->
								<div
									class="grid h-full w-full flex-auto whitespace-pre font-mono text-sm"
									style:grid-template-columns="minmax(auto, max-content) minmax(auto, max-content) 1fr"
								>
									{#each subsection.lines.slice(0, subsection.linesShown) as line}
										<RenderedLine
											{line}
											sectionType={subsection.sectionType}
											filePath={file.path}
										/>
									{/each}
								</div>
								{#if subsection.linesShown < subsection.lines.length}
									<button
										class="text-sm"
										on:click={() => {
											if ('linesShown' in subsection) {
												subsection.linesShown = subsection.lines.length;
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
								{/if}
							{/each}
						</div>
					{:else}
						<div class="">
							<div
								class="grid h-full w-full flex-auto whitespace-pre font-mono text-sm"
								style:grid-template-columns="minmax(auto, max-content) minmax(auto, max-content) 1fr"
							>
								{#each section.lines.slice(0, section.linesShown) as line}
									<RenderedLine {line} sectionType={section.sectionType} filePath={file.path} />
								{/each}
							</div>
						</div>
						{#if section.linesShown < section.lines.length}
							<button
								class="text-sm"
								on:click={() => {
									if ('linesShown' in section) {
										section.linesShown = section.lines.length;
									}
								}}
							>
								{#if idx == 0}
									<IconExpandUp />
								{:else if idx == sections.length - 1}
									<IconExpandDown />
								{:else}
									<IconExpandUpDown />
								{/if}
							</button>
						{/if}
					{/if}
				{/each}
			</div>
		</div>
	{/if}
</div>
