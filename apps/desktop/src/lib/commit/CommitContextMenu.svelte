<script lang="ts">
	import ContextMenu from '$lib/components/contextmenu/ContextMenu.svelte';
	import ContextMenuItem from '$lib/components/contextmenu/ContextMenuItem.svelte';
	import ContextMenuSection from '$lib/components/contextmenu/ContextMenuSection.svelte';
	import { copyToClipboard } from '$lib/utils/clipboard';
	import { openExternalUrl } from '$lib/utils/url';
	import type { Commit, DetailedCommit } from '$lib/vbranches/types';

	interface Props {
		targetElement: HTMLElement | null;
		commit: DetailedCommit | Commit;
		commitUrl: string | undefined;
	}

	const { targetElement, commit, commitUrl }: Props = $props();

	const target = $derived(targetElement ?? undefined);

	let contextMenu = $state<ReturnType<typeof ContextMenu>>();

	export function open(e: MouseEvent) {
		e.preventDefault();
		e.stopPropagation();
		contextMenu?.open(e);
	}

	function copySha() {
		copyToClipboard(commit.id);
		contextMenu?.close();
	}

	function openInBrowser() {
		if (commitUrl) openExternalUrl(commitUrl);
		contextMenu?.close();
	}

	function copyCommitMessage() {
		copyToClipboard(commit.description);
		contextMenu?.close();
	}
</script>

<ContextMenu bind:this={contextMenu} {target} openByMouse>
	<ContextMenuSection>
		<ContextMenuItem label="Copy SHA" onclick={copySha} />
		{#if commitUrl}
			<ContextMenuItem label="Open in browser" onclick={openInBrowser} />
		{/if}
		<ContextMenuItem label="Copy commit message" onclick={copyCommitMessage} />
	</ContextMenuSection>
</ContextMenu>
