<script lang="ts">
	import ContextMenu from '$lib/components/contextmenu/ContextMenu.svelte';
	import ContextMenuItem from '$lib/components/contextmenu/ContextMenuItem.svelte';
	import ContextMenuSection from '$lib/components/contextmenu/ContextMenuSection.svelte';
	import { SETTINGS, type Settings } from '$lib/settings/userSettings';
	import { getEditorUri, openExternalUrl } from '$lib/utils/url';
	import { BranchController } from '$lib/vbranches/branchController';
	import { getContextStoreBySymbol } from '@gitbutler/shared/context';
	import { getContext } from '@gitbutler/shared/context';
	import type { Writable } from 'svelte/store';

	interface Props {
		target: HTMLElement | undefined;
		filePath: string;
		projectPath: string | undefined;
		readonly: boolean;
	}

	let { target, filePath, projectPath, readonly }: Props = $props();

	const branchController = getContext(BranchController);
	const userSettings = getContextStoreBySymbol<Settings, Writable<Settings>>(SETTINGS);

	let contextMenu: ReturnType<typeof ContextMenu> | undefined;

	export function open(e: MouseEvent, item: any) {
		contextMenu?.open(e, item);
	}

	export function close() {
		contextMenu?.close();
	}
</script>

<ContextMenu bind:this={contextMenu} {target} openByMouse>
	{#snippet children(item)}
		<ContextMenuSection>
			{#if item.hunk !== undefined && !readonly}
				<ContextMenuItem
					label="Discard"
					onclick={() => {
						branchController.unapplyHunk(item.hunk);
						contextMenu?.close();
					}}
				/>
			{/if}
			{#if item.lineNumber}
				<ContextMenuItem
					label="Open in {$userSettings.defaultCodeEditor.displayName}"
					onclick={() => {
						if (projectPath) {
							const path = getEditorUri({
								schemeId: $userSettings.defaultCodeEditor.schemeIdentifer,
								path: [projectPath, filePath],
								line: item.lineNumber
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
