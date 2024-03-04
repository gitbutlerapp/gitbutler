<script lang="ts">
	import PopupMenu from '$lib/components/PopupMenu.svelte';
	import ContextMenu from '$lib/components/contextmenu/ContextMenu.svelte';
	import ContextMenuItem from '$lib/components/contextmenu/ContextMenuItem.svelte';
	import ContextMenuSection from '$lib/components/contextmenu/ContextMenuSection.svelte';
	import { open } from '@tauri-apps/api/shell';
	import type { BranchController } from '$lib/vbranches/branchController';

	export let filePath: string;
	export let projectPath: string | undefined;
	export let branchController: BranchController;
	let popupMenu: PopupMenu;

	export function openByMouse(e: MouseEvent, item: any) {
		popupMenu.openByMouse(e, item);
	}
</script>

<PopupMenu bind:this={popupMenu} let:item let:dismiss>
	<ContextMenu>
		<ContextMenuSection>
			{#if item.hunk !== undefined}
				<ContextMenuItem
					label="Discard"
					on:click={() => {
						branchController.unapplyHunk(item.hunk);
						dismiss();
					}}
				/>
			{/if}
			{#if item.lineNumber}
				<ContextMenuItem
					label="Open in VS Code"
					on:mousedown={() => {
						projectPath && open(`vscode://file${projectPath}/${filePath}:${item.lineNumber}`);
						dismiss();
					}}
				/>
			{/if}
		</ContextMenuSection>
	</ContextMenu>
</PopupMenu>
