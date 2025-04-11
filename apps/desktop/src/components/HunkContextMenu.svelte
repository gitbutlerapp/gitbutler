<script lang="ts" module>
	export interface HunkContextItem {
		hunk: Hunk;
		beforeLineNumber: number | undefined;
		afterLineNumber: number | undefined;
		section: ContentSection;
	}
</script>

<script lang="ts">
	import { SETTINGS, type Settings } from '$lib/settings/userSettings';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { getEditorUri, openExternalUrl } from '$lib/utils/url';
	import { getContextStoreBySymbol } from '@gitbutler/shared/context';
	import { getContext } from '@gitbutler/shared/context';
	import ContextMenu from '@gitbutler/ui/ContextMenu.svelte';
	import ContextMenuItem from '@gitbutler/ui/ContextMenuItem.svelte';
	import ContextMenuSection from '@gitbutler/ui/ContextMenuSection.svelte';
	import type { Hunk } from '$lib/hunks/hunk';
	import type { ContentSection } from '$lib/utils/fileSections';
	import type { Writable } from 'svelte/store';

	interface Props {
		trigger: HTMLElement | undefined;
		filePath: string;
		projectPath: string | undefined;
		readonly: boolean;
		projectId: string;
	}

	const { trigger, filePath, projectPath, readonly, projectId }: Props = $props();

	const stackService = getContext(StackService);
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
		<ContextMenuSection>
			{#if item.hunk !== undefined && !readonly}
				<ContextMenuItem
					label="Discard hunk"
					onclick={async () => {
						stackService.legacyUnapplyHunk({ projectId, hunk: item.hunk });
						contextMenu?.close();
					}}
				/>
			{/if}
			{#if item.hunk !== undefined && (item.beforeLineNumber !== undefined || item.afterLineNumber !== undefined) && !readonly}
				<ContextMenuItem
					label={getDiscardLineLabel(item.beforeLineNumber, item.afterLineNumber)}
					onclick={async () => {
						stackService.legacyUnapplyLines({
							projectId,
							hunk: item.hunk,
							linesToUnapply: [{ old: item.beforeLineNumber, new: item.afterLineNumber }]
						});
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
	{/snippet}
</ContextMenu>
