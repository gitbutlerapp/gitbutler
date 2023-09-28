<script lang="ts">
	import PopupMenu from '$lib/components/PopupMenu/PopupMenu.svelte';
	import PopupMenuItem from '$lib/components/PopupMenu/PopupMenuItem.svelte';
	import type { BranchController } from '$lib/vbranches/branchController';
	import type { File } from '$lib/vbranches/types';
	import { open } from '@tauri-apps/api/shell';

	export let file: File;
	export let projectPath: string;
	export let branchController: BranchController;
	let popupMenu: PopupMenu;

	export function openByMouse(e: MouseEvent, item: any) {
		popupMenu.openByMouse(e, item);
	}
</script>

<PopupMenu bind:this={popupMenu} let:item>
	{#if 'expanded' in item.section}
		<PopupMenuItem
			on:click={() => {
				item.section.expanded = false;
			}}
		>
			Collapse
		</PopupMenuItem>
	{/if}
	{#if item.hunk !== undefined && !item.hunk.locked}
		<PopupMenuItem on:click={() => branchController.unapplyHunk(item.hunk)}>Discard</PopupMenuItem>
	{/if}
	{#if item.lineNumber}
		<PopupMenuItem
			on:click={() => open(`vscode://file${projectPath}/${file.path}:${item.lineNumber}`)}
		>
			Open in Visual Studio Code
		</PopupMenuItem>
	{/if}
</PopupMenu>
