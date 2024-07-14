<script lang="ts">
	import ContextMenu from '$lib/components/contextmenu/ContextMenu.svelte';
	import ContextMenuItem from '$lib/components/contextmenu/ContextMenuItem.svelte';
	import ContextMenuSection from '$lib/components/contextmenu/ContextMenuSection.svelte';
	import { editor } from '$lib/editorLink/editorLink';
	import { getContext } from '$lib/utils/context';
	import { BranchController } from '$lib/vbranches/branchController';
	import { open as openFile } from '@tauri-apps/api/shell';

	export let target: HTMLElement;
	export let filePath: string;
	export let projectPath: string | undefined;
	export let readonly: boolean;

	const branchController = getContext(BranchController);

	let contextMenu: ContextMenu;

	export function open(e: MouseEvent, item: any) {
		contextMenu.open(e, item);
	}

	export function close() {
		contextMenu.close();
	}
</script>

<ContextMenu bind:this={contextMenu} {target} openByMouse>
	{#snippet children(item)}
		<ContextMenuSection>
			{#if item.hunk !== undefined && !readonly}
				<ContextMenuItem
					label="Discard"
					on:click={() => {
						branchController.unapplyHunk(item.hunk);
						contextMenu.close();
					}}
				/>
			{/if}
			{#if item.lineNumber}
				<ContextMenuItem
					label="Open in VS Code"
					on:mousedown={() => {
						projectPath &&
							openFile(`${$editor}://file${projectPath}/${filePath}:${item.lineNumber}`);
						contextMenu.close();
					}}
				/>
			{/if}
		</ContextMenuSection>
	{/snippet}
</ContextMenu>
