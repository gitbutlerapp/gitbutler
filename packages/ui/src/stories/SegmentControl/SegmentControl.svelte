<script lang="ts">
	import Segment from '$lib/SegmentControl/Segment.svelte';
	import SegmentControl from '$lib/SegmentControl/SegmentControl.svelte';

	interface Props {
		selectedIndex: number;
		fullWidth?: boolean;
	}

	const { selectedIndex, fullWidth }: Props = $props();
</script>

<SegmentControl
	{selectedIndex}
	{fullWidth}
	onselect={(id) => {
		console.log('Selected index:', id);
	}}
>
	<Segment id="first">First</Segment>
	<Segment id="second">Second</Segment>
	<Segment id="third">Third</Segment>
	<Segment id="fourth">Fourth</Segment>
</SegmentControl>
