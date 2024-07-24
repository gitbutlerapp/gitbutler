<script lang="ts">
	import CopyLinkContextMenu from './CopyLinkContextMenu.svelte';
	import Button from '$lib/shared/Button.svelte';
	import { openExternalUrl } from '$lib/utils/url';

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
