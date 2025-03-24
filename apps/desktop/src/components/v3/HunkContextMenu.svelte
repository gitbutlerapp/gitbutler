<script lang="ts" module>
	export interface HunkContextItem {
		hunk: DiffHunk;
		beforeLineNumber: number | undefined;
		afterLineNumber: number | undefined;
	}

	export function isHunkContextItem(item: unknown): item is HunkContextItem {
		return typeof item === 'object' && item !== null && 'hunk' in item && isDiffHunk(item.hunk);
	}
</script>

<script lang="ts">
	import { isDiffHunk, type DiffHunk } from '$lib/hunks/hunk';
	import { showInfo } from '$lib/notifications/toasts';
	import { SETTINGS, type Settings } from '$lib/settings/userSettings';
	import { getEditorUri, openExternalUrl } from '$lib/utils/url';
	import { getContextStoreBySymbol } from '@gitbutler/shared/context';
	import ContextMenu from '@gitbutler/ui/ContextMenu.svelte';
	import ContextMenuItem from '@gitbutler/ui/ContextMenuItem.svelte';
	import ContextMenuSection from '@gitbutler/ui/ContextMenuSection.svelte';
	import type { Writable } from 'svelte/store';

	interface Props {
		trigger: HTMLElement | undefined;
		filePath: string;
		projectPath: string | undefined;
		readonly: boolean;
	}

	const { trigger, filePath, projectPath, readonly }: Props = $props();

	const userSettings = getContextStoreBySymbol<Settings, Writable<Settings>>(SETTINGS);

	let contextMenu: ReturnType<typeof ContextMenu> | undefined;

	function getDiscardLineLabel(
		beforeLineNumber: number | undefined,
		afterLineNumber: number | undefined
	) {
		if (beforeLineNumber !== undefined && afterLineNumber !== undefined)
			return `Discard line ${beforeLineNumber} -> ${afterLineNumber}`;
		if (beforeLineNumber !== undefined) return `Discard old line ${beforeLineNumber}`;

		if (afterLineNumber !== undefined) return `Discard new line ${afterLineNumber}`;

		return 'Discard line';
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
				{#if item.hunk !== undefined && !readonly}
					<ContextMenuItem
						label="Discard hunk"
						disabled
						onclick={() => {
							// branchController.unapplyHunk(item.hunk);
							showInfo('Woops', 'Discard hunk not implemented');
							contextMenu?.close();
						}}
					/>
				{/if}
				{#if item.hunk !== undefined && (item.beforeLineNumber !== undefined || item.afterLineNumber !== undefined) && !readonly}
					<ContextMenuItem
						label={getDiscardLineLabel(item.beforeLineNumber, item.afterLineNumber)}
						disabled
						onclick={() => {
							// branchController.unapplyLines(item.hunk, [
							// 	{ old: item.beforeLineNumber, new: item.afterLineNumber }
							// ]);
							showInfo('Woops', 'Discard line not implemented');
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
