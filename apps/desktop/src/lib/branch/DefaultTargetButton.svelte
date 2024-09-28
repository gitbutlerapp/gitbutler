<script lang="ts">
	import Button from '@gitbutler/ui/Button.svelte';

	interface Props {
		selectedForChanges: boolean;
		onclick: () => void;
	}

	const { selectedForChanges, onclick }: Props = $props();
</script>

{#if selectedForChanges}
	<Button
		style="pop"
		kind="soft"
		tooltip="New changes will land here"
		icon="target"
		size="tag"
		clickable={false}
	>
		Default branch
	</Button>
{:else}
	<Button
		style="ghost"
		outline
		tooltip="When selected, new changes land here"
		icon="target"
		size="tag"
		{onclick}
	>
		Set as default
	</Button>
{/if}
