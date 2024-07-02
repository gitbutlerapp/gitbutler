<script lang="ts">
	import ContextMenu from '$lib/components/contextmenu/ContextMenu.svelte';
	import ContextMenuItem from '$lib/components/contextmenu/ContextMenuItem.svelte';
	import ContextMenuSection from '$lib/components/contextmenu/ContextMenuSection.svelte';
	import PopupMenu from '$lib/shared/PopupMenu.svelte';
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
