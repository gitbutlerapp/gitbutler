<script lang="ts">
	import CopyLinkContextMenu from './CopyLinkContextMenu.svelte';
	import { openExternalUrl } from '$lib/utils/url';
	import Button from '@gitbutler/ui/inputs/Button.svelte';

	const { url }: { url: string } = $props();

	let copyLinkContextMenu = $state<CopyLinkContextMenu>();
	let viewPrButton = $state<HTMLElement>();
</script>

<Button
	size="tag"
	icon="open-link"
	style="ghost"
	outline
	shrinkable
	bind:el={viewPrButton}
	on:click={(e) => {
		openExternalUrl(url);
		e.preventDefault();
		e.stopPropagation();
	}}
	on:contextmenu={(e) => {
		e.preventDefault();
		copyLinkContextMenu?.openByMouse(e);
	}}
>
	Open in browser
</Button>
<CopyLinkContextMenu bind:this={copyLinkContextMenu} target={viewPrButton} {url} />
