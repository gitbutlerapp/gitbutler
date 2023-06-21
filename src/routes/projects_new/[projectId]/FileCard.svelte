<script lang="ts">
	import { createEventDispatcher } from 'svelte';
	import { dndzone } from 'svelte-dnd-action';
	import { formatDistanceToNow, compareDesc } from 'date-fns';
	import type { DndEvent } from 'svelte-dnd-action/typings';
	import type { Hunk } from './types';
	import HunkDiffViewer from './HunkDiffViewer.svelte';
	import { summarizeHunk } from '$lib/summaries';
	import { IconTriangleUp, IconTriangleDown } from '$lib/icons';

	export let filepath: string;
	export let hunks: Hunk[];
	let zoneEl: HTMLElement;

	const dispatch = createEventDispatcher();
	export let expanded = true;

	function handleDndEvent(e: CustomEvent<DndEvent<Hunk>>) {
		hunks = e.detail.items;
		hunks.sort((itemA, itemB) => compareDesc(itemA.modifiedAt, itemB.modifiedAt));
		if (e.type == 'finalize' && hunks.length == 0) dispatch('empty');
	}

	function hunkSize(hunk: string): number[] {
		const linesAdded = hunk.split('\n').filter((line) => line.startsWith('+')).length;
		const linesRemoved = hunk.split('\n').filter((line) => line.startsWith('-')).length;
		return [linesAdded, linesRemoved];
	}

	function boldenFilename(filepath: string): string {
		const parts = filepath.split('/');
		if (parts.length == 0) return '';
		return (
			parts.slice(0, -2).join('/') +
			'/<span class="font-bold">' +
			parts[parts.length - 1] +
			'</span>'
		);
	}
</script>

<div
	class="changed-file gb-border-2 gb-bg-1 gb-text-2 flex w-full flex-col justify-center gap-2 rounded-lg border p-2"
>
	<div class="flex items-center gap-2">
		<div class="flex-grow overflow-hidden text-ellipsis whitespace-nowrap" title={filepath}>
			{@html boldenFilename(filepath)}
		</div>
		<div
			on:click={() => (expanded = !expanded)}
			on:keypress={() => (expanded = !expanded)}
			class="cursor-pointer p-2"
		>
			{#if expanded}
				<IconTriangleUp />
			{:else}
				<IconTriangleDown />
			{/if}
		</div>
	</div>

	<div
		class="hunk-change-container flex flex-col gap-2 rounded"
		bind:this={zoneEl}
		use:dndzone={{
			items: hunks,
			zoneTabIndex: -1,
			autoAriaDisabled: true,
			types: ['hunk', filepath],
			receives: [filepath]
		}}
		on:consider={handleDndEvent}
		on:finalize={handleDndEvent}
	>
		{#if expanded}
			{#each hunks || [] as hunk (hunk.id)}
				<div class="changed-hunk gb-border-2 flex w-full flex-col gap-1 rounded-sm border">
					<div class="w-full text-ellipsis p-2">
						{#await summarizeHunk(hunk.diff) then description}
							{description}
						{/await}
					</div>
					<div class="gb-border-2 cursor-pointer border-t border-b text-sm">
						<!-- Disabling syntax highlighting for perormance reasons -->
						<HunkDiffViewer diff={hunk.diff} filePath="foo" linesShown={2} />
					</div>
					<div class="flex p-2 text-sm">
						<div class="flex flex-grow gap-1">
							<div class="text-green-600">+{hunkSize(hunk.diff)[0]}</div>
							{#if hunkSize(hunk.diff)[1] > 0}
								<div class="text-red-600">-{hunkSize(hunk.diff)[1]}</div>
							{/if}
						</div>
						<div class="text-right text-zinc-400">
							{formatDistanceToNow(hunk.modifiedAt, { addSuffix: true })}
						</div>
					</div>
				</div>
			{/each}
		{/if}
	</div>
</div>
