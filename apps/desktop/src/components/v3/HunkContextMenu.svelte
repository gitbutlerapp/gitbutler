<script lang="ts" module>
	export interface HunkContextItem {
		hunk: DiffHunk;
		selectedLines: LineId[] | undefined;
		beforeLineNumber: number | undefined;
		afterLineNumber: number | undefined;
	}

	export function isHunkContextItem(item: unknown): item is HunkContextItem {
		return typeof item === 'object' && item !== null && 'hunk' in item && isDiffHunk(item.hunk);
	}
</script>

<script lang="ts">
	import { isDiffHunk, lineIdsToHunkHeaders, type DiffHunk } from '$lib/hunks/hunk';
	import { SETTINGS, type Settings } from '$lib/settings/userSettings';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { getEditorUri, openExternalUrl } from '$lib/utils/url';
	import { getContextStoreBySymbol, inject } from '@gitbutler/shared/context';
	import ContextMenu from '@gitbutler/ui/ContextMenu.svelte';
	import ContextMenuItem from '@gitbutler/ui/ContextMenuItem.svelte';
	import ContextMenuSection from '@gitbutler/ui/ContextMenuSection.svelte';
	import type { TreeChange } from '$lib/hunks/change';
	import type { LineId } from '@gitbutler/ui/utils/diffParsing';
	import type { Writable } from 'svelte/store';

	interface Props {
		trigger: HTMLElement | undefined;
		projectId: string;
		change: TreeChange;
		projectPath: string | undefined;
		readonly: boolean;
		unSelectHunk: (hunk: DiffHunk) => void;
	}

	const { trigger, projectId, change, projectPath, readonly, unSelectHunk }: Props = $props();

	const [stackService] = inject(StackService);
	const userSettings = getContextStoreBySymbol<Settings, Writable<Settings>>(SETTINGS);
	const [discardChanges] = stackService.discardChanges;

	const filePath = $derived(change.path);
	let contextMenu: ReturnType<typeof ContextMenu> | undefined;

	function getDiscardLineLabel(item: HunkContextItem) {
		const { selectedLines } = item;

		if (selectedLines !== undefined && selectedLines.length > 0)
			return `Discard ${selectedLines.length} selected lines`;

		return '';
	}

	async function discardHunk(item: HunkContextItem) {
		const previousPathBytes =
			change.status.type === 'Rename' ? change.status.subject.previousPath : null;

		await discardChanges({
			projectId,
			worktreeChanges: [
				{
					previousPathBytes,
					pathBytes: change.path,
					hunkHeaders: [item.hunk]
				}
			]
		});

		unSelectHunk(item.hunk);
	}

	async function discardHunkLines(item: HunkContextItem) {
		if (item.selectedLines === undefined || item.selectedLines.length === 0) return;
		const previousPathBytes =
			change.status.type === 'Rename' ? change.status.subject.previousPath : null;

		await discardChanges({
			projectId,
			worktreeChanges: [
				{
					previousPathBytes,
					pathBytes: change.path,
					hunkHeaders: lineIdsToHunkHeaders(item.selectedLines, item.hunk.diff)
				}
			]
		});

		unSelectHunk(item.hunk);
	}

	export function open(e: MouseEvent, item: HunkContextItem) {
		contextMenu?.open(e, item);
	}

	export function close() {
		contextMenu?.close();
	}
</script>

<ContextMenu bind:this={contextMenu} rightClickTrigger={trigger}>
	{#snippet children(item)}
		{#if isHunkContextItem(item)}
			<ContextMenuSection>
				{#if !readonly}
					<ContextMenuItem
						label="Discard hunk"
						onclick={() => {
							discardHunk(item);
							contextMenu?.close();
						}}
					/>
				{/if}
				{#if item.selectedLines !== undefined && item.selectedLines.length > 0 && !readonly}
					<ContextMenuItem
						label={getDiscardLineLabel(item)}
						onclick={() => {
							discardHunkLines(item);
							contextMenu?.close();
						}}
					/>
				{/if}
				{#if item.beforeLineNumber !== undefined || item.afterLineNumber !== undefined}
					<ContextMenuItem
						label="Open in {$userSettings.defaultCodeEditor.displayName}"
						onclick={() => {
							if (projectPath) {
								const path = getEditorUri({
									schemeId: $userSettings.defaultCodeEditor.schemeIdentifer,
									path: [projectPath, filePath],
									line: item.beforeLineNumber ?? item.afterLineNumber
								});
								openExternalUrl(path);
							}
							contextMenu?.close();
						}}
					/>
				{/if}
			</ContextMenuSection>
		{:else}
			{'Malformed item :('}
		{/if}
	{/snippet}
</ContextMenu>
