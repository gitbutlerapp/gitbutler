<script lang="ts">
	import ContextMenuItem from '@gitbutler/ui/ContextMenuItem.svelte';
	import ContextMenuSection from '@gitbutler/ui/ContextMenuSection.svelte';
	import DropDownButton from '@gitbutler/ui/DropDownButton.svelte';
	import type { Patch } from '@gitbutler/shared/branches/types';

	interface Props {
		patch: Patch;
	}

	const { patch }: Props = $props();

	function approve() {
		console.log('Approve patch', patch.changeId);
	}

	function requestChanges() {
		console.log('Request changes for patch', patch.changeId);
	}
</script>

<DropDownButton icon="success" style="pop" type="submit">
	Approve commit
	{#snippet contextMenuSlot()}
		<ContextMenuSection>
			<ContextMenuItem label="Approve commit" onclick={approve} />
			<ContextMenuItem label="Request changes" onclick={requestChanges} />
		</ContextMenuSection>
	{/snippet}
</DropDownButton>
