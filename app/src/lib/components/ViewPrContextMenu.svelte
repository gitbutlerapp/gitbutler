<script lang="ts">
	import PopupMenu from '$lib/components/PopupMenu.svelte';
	import ContextMenu from '$lib/components/contextmenu/ContextMenu.svelte';
	import ContextMenuItem from '$lib/components/contextmenu/ContextMenuItem.svelte';
	import ContextMenuSection from '$lib/components/contextmenu/ContextMenuSection.svelte';
	import { copyToClipboard } from '$lib/utils/clipboard';

	export let prUrl: string;
	let popupMenu: PopupMenu;

	export function openByMouse(e: MouseEvent, item: any) {
		popupMenu.openByMouse(e, item);
	}
</script>

<PopupMenu bind:this={popupMenu} let:dismiss>
	<ContextMenu>
		<ContextMenuSection>
			<ContextMenuItem
				label="Copy to Clipbaord"
				on:click={() => {
					copyToClipboard(prUrl);
					dismiss();
				}}
			/>
		</ContextMenuSection>
	</ContextMenu>
</PopupMenu>
