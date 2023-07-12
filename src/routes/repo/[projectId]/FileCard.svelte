<script lang="ts">
	import { createEventDispatcher } from 'svelte';
	import { formatDistanceToNow } from 'date-fns';
	import type { Hunk } from '$lib/vbranches';
	import HunkDiffViewer from './HunkDiffViewer.svelte';
	import { summarizeHunk } from '$lib/summaries';
	import { IconTriangleUp, IconTriangleDown } from '$lib/icons';
	import { open } from '@tauri-apps/api/shell';
	import PopupMenu from '$lib/components/PopupMenu/PopupMenu.svelte';
	import PopupMenuItem from '$lib/components/PopupMenu/PopupMenuItem.svelte';
	import { SETTINGS_CONTEXT, type SettingsStore } from '$lib/userSettings';
	import { getContext } from 'svelte';
	import { dzTrigger } from './dropZone';

	const userSettings = getContext<SettingsStore>(SETTINGS_CONTEXT);

	export let id: string;
	export let projectPath: string;
	export let filepath: string;
	export let hunks: Hunk[];
	export let maximized: boolean;
	export let dzType: string;

	const dispatch = createEventDispatcher<{
		expanded: boolean;
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
			parts.slice(0, -1).join('/') +
			'/<span class="font-bold text-light-800 dark:text-dark-50">' +
			parts[parts.length - 1] +
			'</span>'
		);
	}

	// This should be refactored, it's borrowed from HunkDiffViewer
	function getFirstLineNumber(diff: string): number {
		return parseInt(diff.split('\n')[0].split('@@')[1].trim().split(' ')[0].split(',')[0].slice(1));
	}

	function getAllHunksOwnership(): string {
		return id + ':' + hunks.map((h) => h.id).join(',');
	}
</script>

<div
	draggable="true"
	use:dzTrigger={{ type: dzType }}
	on:dragstart={(e) => e.dataTransfer?.setData('text/hunk', getAllHunksOwnership())}
	class="changed-file inner"
>
	<div
		class="flex w-full flex-col justify-center gap-2 rounded border border-light-300 bg-light-50 text-light-900 dark:border-dark-400 dark:bg-dark-700 dark:text-light-300"
	>
		<div class="flex px-2 pt-2">
			<div
				class="flex-grow overflow-hidden text-ellipsis whitespace-nowrap text-light-800 dark:text-dark-100"
				title={filepath}
			>
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
						use:dzTrigger={{ type: dzType }}
						on:dragstart={(e) => e.dataTransfer?.setData('text/hunk', id + ':' + hunk.id)}
						on:contextmenu|preventDefault={(e) => popupMenu.openByMouse(e, hunk)}
						class="changed-hunk "
					>
						<div
							class="flex w-full flex-col overflow-hidden rounded border border-light-200 bg-white dark:border-dark-400 dark:bg-dark-900"
						>
							{#if $userSettings.aiSummariesEnabled}
								<div class="truncate whitespace-normal p-2">
									{#await summarizeHunk(hunk.diff) then description}
										{description}
									{/await}
								</div>
							{/if}
							<div class="cursor-pointer overflow-clip">
								<HunkDiffViewer
									diff={hunk.diff}
									filePath={hunk.filePath}
									linesShown={maximized ? 8 : 2}
									{userSettings}
								/>
							</div>
							<div class="flex px-2 py-1">
								<div class="flex flex-grow gap-1">
									<div class="text-green-600">+{hunkSize(hunk.diff)[0]}</div>
									{#if hunkSize(hunk.diff)[1] > 0}
										<div class="text-red-600">-{hunkSize(hunk.diff)[1]}</div>
									{/if}
								</div>
								<div class="text-right text-light-700 dark:text-dark-200">
									{formatDistanceToNow(hunk.modifiedAt, { addSuffix: true })}
								</div>
							</div>
						</div>
					</div>
				{/each}
			{/if}
		</div>
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
