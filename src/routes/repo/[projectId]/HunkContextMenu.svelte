<script lang="ts">
	import PopupMenu from '$lib/components/PopupMenu/PopupMenu.svelte';
	import PopupMenuItem from '$lib/components/PopupMenu/PopupMenuItem.svelte';
	import type { File } from '$lib/vbranches/types';
	import { open } from '@tauri-apps/api/shell';

	export let file: File;
	export let projectPath: string;
	let popupMenu: PopupMenu;

	export function openByMouse(e: MouseEvent, item: any) {
		console.log(popupMenu);
		popupMenu.openByMouse(e, item);
	}
</script>

<PopupMenu bind:this={popupMenu} let:item>
	<PopupMenuItem
		on:click={() => {
			if ('expanded' in item.section) {
				item.section.expanded = false;
				// sections = sections;
			}
		}}
	>
		Collapse
	</PopupMenuItem>
	<PopupMenuItem
		on:click={() => open(`vscode://file${projectPath}/${file.path}:${item.lineNumber}`)}
	>
		Open in Visual Studio Code
	</PopupMenuItem>
</PopupMenu>
