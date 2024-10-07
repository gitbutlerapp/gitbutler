<script lang="ts">
	import Button from '@gitbutler/ui/Button.svelte';

	interface Props {
		selectedForChanges: boolean;
		onclick: () => void;
		size: 'button' | 'tag';
	}

	const { selectedForChanges, size = 'button', onclick }: Props = $props();
</script>

{#if selectedForChanges}
	<Button
		style="pop"
		kind="soft"
		tooltip="New changes will land here"
		icon="target"
		{size}
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
		{size}
		{onclick}
	>
		Set as default
	</Button>
{/if}
