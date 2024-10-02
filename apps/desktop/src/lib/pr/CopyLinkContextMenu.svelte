<script lang="ts">
	import ContextMenu from '$lib/components/contextmenu/ContextMenu.svelte';
	import ContextMenuItem from '$lib/components/contextmenu/ContextMenuItem.svelte';
	import ContextMenuSection from '$lib/components/contextmenu/ContextMenuSection.svelte';
	import { copyToClipboard } from '$lib/utils/clipboard';

	const { target, url }: { target: HTMLElement | undefined; url: string } = $props();

	let menu: ReturnType<typeof ContextMenu>;

	export function openByMouse(e: MouseEvent) {
		menu.open(e);
	}
</script>

<ContextMenu bind:this={menu} openByMouse {target}>
	<ContextMenuSection>
		<ContextMenuItem
			label="Copy to Clipbaord"
			on:click={() => {
				copyToClipboard(url);
				menu.close();
			}}
		/>
	</ContextMenuSection>
</ContextMenu>
