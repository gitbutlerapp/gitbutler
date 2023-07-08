<script lang="ts">
	import { createEventDispatcher } from 'svelte';
	import { formatDistanceToNow } from 'date-fns';
	import type { Hunk } from '$lib/api/ipc/vbranches';
	import HunkDiffViewer from './HunkDiffViewer.svelte';
	import { summarizeHunk } from '$lib/summaries';
	import { IconTriangleUp, IconTriangleDown } from '$lib/icons';
	import { open } from '@tauri-apps/api/shell';
	import PopupMenu from '$lib/components/PopupMenu/PopupMenu.svelte';
	import PopupMenuItem from '$lib/components/PopupMenu/PopupMenuItem.svelte';
	import { SETTINGS_CONTEXT, type SettingsStore } from '$lib/userSettings';
	import { getContext } from 'svelte';

	const userSettings = getContext<SettingsStore>(SETTINGS_CONTEXT);

	export let id: string;
	export let projectPath: string;
	export let filepath: string;
	export let hunks: Hunk[];
	export let maximized: boolean;

	const dispatch = createEventDispatcher<{
		expanded: boolean;
		update: Hunk[];
		drag: boolean;
	}>();
	export let expanded: boolean | undefined;

	let popupMenu: PopupMenu;

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

	// This should be refactored, it's borrowed from HunkDiffViewer
	function getFirstLineNumber(diff: string): number {
		return parseInt(diff.split('\n')[0].split('@@')[1].trim().split(' ')[0].split(',')[0].slice(1));
	}
</script>

<div
	draggable="true"
	on:dragstart|stopPropagation={(e) => {
		if (!e.dataTransfer) return;
		e.dataTransfer.setData('text/hunk', id + ':' + hunks.map((h) => h.id).join(','));
		dispatch('drag', true);
		return true;
	}}
	on:dragend|stopPropagation={(e) => {
		dispatch('drag', false);
	}}
	class="changed-file flex w-full flex-col justify-center gap-2 rounded-lg border border-light-300 bg-light-50 text-light-900 dark:border-dark-400 dark:bg-dark-700 dark:text-light-300"
>
	<div class="items-cente flex px-2 pt-2">
		<div class="flex-grow overflow-hidden text-ellipsis whitespace-nowrap " title={filepath}>
			{@html boldenFilename(filepath)}
		</div>
		<div
			on:click={() => {
				expanded = !expanded;
				dispatch('expanded', expanded);
			}}
			on:keypress={() => (expanded = !expanded)}
			class="cursor-pointer p-2 text-light-600 dark:text-dark-200"
		>
			{#if expanded}
				<IconTriangleUp />
			{:else}
				<IconTriangleDown />
			{/if}
		</div>
	</div>

	<div class="hunk-change-container flex flex-col gap-2 rounded px-2 pb-2">
		{#if expanded}
			{#each hunks || [] as hunk (hunk.id)}
				<div
					draggable="true"
					on:dragstart|stopPropagation={(e) => {
						if (!e.dataTransfer) return;
						e.dataTransfer.setData('text/hunk', id + ':' + hunk.id);
						dispatch('drag', true);
						return false;
					}}
					on:dragend|stopPropagation={(e) => {
						dispatch('drag', false);
					}}
					on:contextmenu|preventDefault={(e) => popupMenu.openByMouse(e, hunk)}
					class="changed-hunk flex w-full flex-col rounded-lg border border-light-200 bg-white dark:border-dark-400 dark:bg-dark-900"
				>
					{#if $userSettings.aiSummariesEnabled}
						<div class="truncate whitespace-normal p-2">
							{#await summarizeHunk(hunk.diff) then description}
								{description}
							{/await}
						</div>
					{/if}
					<div class="cursor-pointer overflow-clip text-sm">
						<!-- Disabling syntax highlighting for performance reasons -->
						<HunkDiffViewer diff={hunk.diff} filePath="foo" linesShown={maximized ? 8 : 2} />
					</div>
					<div class="flex px-2 py-1 text-sm">
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
	<PopupMenu bind:this={popupMenu} let:item={hunk}>
		<PopupMenuItem
			on:click={() =>
				open(`vscode://file${projectPath}/${filepath}:${getFirstLineNumber(hunk.diff)}`)}
		>
			Open in VS Code
		</PopupMenuItem>
	</PopupMenu>
</div>
