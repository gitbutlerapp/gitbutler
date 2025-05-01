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
	import { ircEnabled } from '$lib/config/uiFeatureFlags';
	import { isDiffHunk, lineIdsToHunkHeaders, type DiffHunk } from '$lib/hunks/hunk';
	import { IrcService } from '$lib/irc/ircService.svelte';
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

	const [stackService, ircService] = inject(StackService, IrcService);

	const userSettings = getContextStoreBySymbol<Settings, Writable<Settings>>(SETTINGS);
	const ircChats = $derived(ircService.getChats());
	const ircUsers = $derived(Object.keys(ircChats));
	const ircChannels = $derived(Object.keys(ircService.getChannels()));

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
			change.status.type === 'Rename' ? change.status.subject.previousPathBytes : null;

		unSelectHunk(item.hunk);

		await stackService.discardChanges({
			projectId,
			worktreeChanges: [
				{
					previousPathBytes,
					pathBytes: change.pathBytes,
					hunkHeaders: [item.hunk]
				}
			]
		});
	}

	async function discardHunkLines(item: HunkContextItem) {
		if (item.selectedLines === undefined || item.selectedLines.length === 0) return;
		const previousPathBytes =
			change.status.type === 'Rename' ? change.status.subject.previousPathBytes : null;

		unSelectHunk(item.hunk);

		await stackService.discardChanges({
			projectId,
			worktreeChanges: [
				{
					previousPathBytes,
					pathBytes: change.pathBytes,
					hunkHeaders: lineIdsToHunkHeaders(item.selectedLines, item.hunk.diff, 'discard')
				}
			]
		});
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
				{#if $ircEnabled}
					{#each ircUsers as ircUser}
						<ContextMenuItem
							label={ircUser}
							onclick={() => {
								const data = btoa(JSON.stringify({ change, diff: item.hunk }));
								if (!data) return;
								ircService.sendToNick(ircUser, change.path, data);
								contextMenu?.close();
							}}
						/>
					{/each}
					{#each ircChannels as ircChannel}
						<ContextMenuItem
							label={ircChannel}
							onclick={() => {
								const data = btoa(JSON.stringify({ change, diff: item.hunk }));
								if (!data) return;
								ircService.sendToGroup(ircChannel, change.path, data);
								contextMenu?.close();
							}}
						/>
					{/each}
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
