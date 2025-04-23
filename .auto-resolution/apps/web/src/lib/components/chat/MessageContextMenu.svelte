<script lang="ts">
	import { copyToClipboard } from '@gitbutler/shared/clipboard';
	import ContextMenu from '@gitbutler/ui/ContextMenu.svelte';
	import ContextMenuItem from '@gitbutler/ui/ContextMenuItem.svelte';
	import ContextMenuSection from '@gitbutler/ui/ContextMenuSection.svelte';

	type Props = {
		menu: ReturnType<typeof ContextMenu> | undefined;
		leftClickTrigger: HTMLElement | undefined;
		messageId: string;
		onToggle?: (isOpen: boolean, isLeftClick: boolean) => void;
	};

	let { menu = $bindable(), leftClickTrigger, messageId, onToggle }: Props = $props();

	function copyLink() {
		const url = new URL(window.location.href);
		url.searchParams.set('m', messageId);
		copyToClipboard(url.toString());
		menu?.close();
	}
</script>

<ContextMenu bind:this={menu} {leftClickTrigger} ontoggle={onToggle}>
	<ContextMenuSection>
		<ContextMenuItem label="Copy link" onclick={copyLink} />
	</ContextMenuSection>
</ContextMenu>
