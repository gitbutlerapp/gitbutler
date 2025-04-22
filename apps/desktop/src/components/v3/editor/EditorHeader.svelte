<script lang="ts">
	import Segment from '@gitbutler/ui/segmentControl/Segment.svelte';
	import SegmentControl from '@gitbutler/ui/segmentControl/SegmentControl.svelte';

	interface Props {
		title: string;
		markdown: boolean;
	}

	let { title, markdown = $bindable() }: Props = $props();
</script>

<div class="editor-header">
	<h1 class="text-14 text-semibold">{title}</h1>

	<SegmentControl
		defaultIndex={markdown ? 1 : 0}
		onselect={(id) => {
			markdown = id === 'richtext';
		}}
	>
		<Segment id="plain">Plain</Segment>
		<Segment id="richtext">Rich-text Editor</Segment>
	</SegmentControl>
</div>

<style lang="postcss">
	.editor-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		background-color: var(--clr-bg-1);
		padding: 12px 12px 12px 16px;
		border-bottom: 1px solid var(--clr-border-2);
	}
</style>
