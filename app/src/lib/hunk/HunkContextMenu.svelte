<script lang="ts">
	import ContextMenu from '$lib/components/contextmenu/ContextMenu.svelte';
	import ContextMenuItem from '$lib/components/contextmenu/ContextMenuItem.svelte';
	import ContextMenuSection from '$lib/components/contextmenu/ContextMenuSection.svelte';
	import PopupMenu from '$lib/shared/PopupMenu.svelte';
	import { getContext } from '$lib/utils/context';
	import { editor } from '$lib/utils/systemEditor';
	import { BranchController } from '$lib/vbranches/branchController';
	import { open } from '@tauri-apps/api/shell';

	export let filePath: string;
	export let projectPath: string | undefined;
	export let readonly: boolean;

	const branchController = getContext(BranchController);

	let popupMenu: PopupMenu;

	export function openByMouse(e: MouseEvent, item: any) {
		popupMenu.openByMouse(e, item);
	}
</script>

<PopupMenu bind:this={popupMenu} let:item let:dismiss>
	<ContextMenu>
		<ContextMenuSection>
			{#if item.hunk !== undefined && !readonly}
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
						projectPath &&
							open(`${editor.get()}://file${projectPath}/${filePath}:${item.lineNumber}`);
						dismiss();
					}}
				/>
			{/if}
		</ContextMenuSection>
	</ContextMenu>
</PopupMenu>
