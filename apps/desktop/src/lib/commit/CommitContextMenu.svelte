<script lang="ts">
	// import ContextMenu from '$lib/components/contextmenu/ContextMenu.svelte';
	import ContextMenuItem from '$lib/components/contextmenu/ContextMenuItem.svelte';
	import ContextMenuSection from '$lib/components/contextmenu/ContextMenuSection.svelte';
	import { copyToClipboard } from '$lib/utils/clipboard';
	import { openExternalUrl } from '$lib/utils/url';
	import type { Commit, DetailedCommit } from '$lib/vbranches/types';

	interface Props {
		commit: DetailedCommit | Commit;
		commitUrl: string | undefined;
	}

	const { commit, commitUrl }: Props = $props();
</script>

<!-- <ContextMenu bind:this={contextMenu} {target} {openByMouse} {onopen} {onclose}> -->
<ContextMenuSection>
	{#if commitUrl}
		<ContextMenuItem
			label="Open in browser"
			onclick={async () => await openExternalUrl(commitUrl)}
		/>
		<ContextMenuItem label="Copy commit link" onclick={() => copyToClipboard(commitUrl)} />
	{/if}
	<ContextMenuItem
		label="Copy commit message"
		onclick={() => copyToClipboard(commit.description)}
	/>
</ContextMenuSection>
<ContextMenuSection>
	<ContextMenuItem label="Add empty commit above" onclick={() => {}} />
	<ContextMenuItem label="Add empty commit below" onclick={() => {}} />
</ContextMenuSection>
<!-- </ContextMenu> -->
