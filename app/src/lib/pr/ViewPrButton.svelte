<script lang="ts">
	import CopyLinkContextMenu from './CopyLinkContextMenu.svelte';
	import Button from '$lib/shared/Button.svelte';
	import { openExternalUrl } from '$lib/utils/url';

	const { url }: { url: string } = $props();
</script>

<Button
	size="tag"
	icon="open-link"
	style="ghost"
	outline
	shrinkable
	onclick={(e) => {
		openExternalUrl(url);
		e.preventDefault();
		e.stopPropagation();
	}}
>
	Open in browser
	{#snippet menu(target)}
		<CopyLinkContextMenu {target} {url} />
	{/snippet}
</Button>
