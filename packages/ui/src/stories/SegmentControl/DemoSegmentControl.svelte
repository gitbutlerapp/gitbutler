<script lang="ts">
	import Segment from '$lib/SegmentControl/Segment.svelte';
	import SegmentControl from '$lib/SegmentControl/SegmentControl.svelte';

	interface Props {
		defaultIndex: number;
		fullWidth?: boolean;
		segments: {
			id: string;
			label: string;
		}[];
	}

	const { defaultIndex, fullWidth, segments }: Props = $props();
</script>

<div class="wrapper">
	<SegmentControl
		{defaultIndex}
		{fullWidth}
		onselect={(id) => {
			console.log('Selected index:', id);
		}}
	>
		{#each segments as segment}
			<Segment id={segment.id}>{segment.label}</Segment>
		{/each}
	</SegmentControl>
</div>
