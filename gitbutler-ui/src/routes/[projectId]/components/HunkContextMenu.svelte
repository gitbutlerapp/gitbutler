<script lang="ts">
	import PopupMenu from '$lib/components/PopupMenu.svelte';
	import ContextMenu from '$lib/components/contextmenu/ContextMenu.svelte';
	import ContextMenuItem from '$lib/components/contextmenu/ContextMenuItem.svelte';
	import ContextMenuSection from '$lib/components/contextmenu/ContextMenuSection.svelte';
	import type { BranchController } from '$lib/vbranches/branchController';
	import type { File } from '$lib/vbranches/types';
	import { open } from '@tauri-apps/api/shell';

	export let file: File;
	export let projectPath: string | undefined;
	export let branchController: BranchController;
	let popupMenu: PopupMenu;

	export function openByMouse(e: MouseEvent, item: any) {
		popupMenu.openByMouse(e, item);
	}
</script>

<PopupMenu bind:this={popupMenu} let:item>
	<ContextMenu>
		<ContextMenuSection>
			{#if item.hunk !== undefined}
				<ContextMenuItem label="Discard" on:click={() => branchController.unapplyHunk(item.hunk)} />
			{/if}
			{#if item.lineNumber}
				<ContextMenuItem
					label="Open in VS Code"
					on:click={() =>
						projectPath && open(`vscode://file${projectPath}/${file.path}:${item.lineNumber}`)}
				/>
			{/if}
		</ContextMenuSection>
	</ContextMenu>
</PopupMenu>
