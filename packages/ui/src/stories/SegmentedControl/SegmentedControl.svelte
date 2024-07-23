<script lang="ts">
	import Segment from '$lib/SegmentedControl/Segment.svelte';
	import SegmentedControl from '$lib/SegmentedControl/SegmentedControl.svelte';

	interface Props {
		selectedIndex: number;
	}

	const { selectedIndex }: Props = $props();
</script>

<SegmentedControl
	{selectedIndex}
	onselect={(id) => {
		console.log('Selected index:', id);
	}}
>
	<Segment id="first">First</Segment>
	<Segment id="second">Second</Segment>
	<Segment id="third">Third</Segment>
	<Segment id="fourth">Fourth</Segment>
</SegmentedControl>
