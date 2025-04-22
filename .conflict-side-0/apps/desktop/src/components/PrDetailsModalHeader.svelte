<script lang="ts">
	import Segment from '@gitbutler/ui/segmentControl/Segment.svelte';
	import SegmentControl from '@gitbutler/ui/segmentControl/SegmentControl.svelte';

	interface Props {
		isEditing: boolean;
	}

	let { isEditing = $bindable() }: Props = $props();
</script>

<!-- MAIN -->
<div class="pr-header">
	<div class="pr-header__row">
		<h3 class="text-14 text-body text-semibold pr-title">Submit code for review</h3>

		<SegmentControl
			defaultIndex={0}
			onselect={(id) => {
				isEditing = id === 'write';
			}}
		>
			<Segment id="write">Edit</Segment>
			<Segment id="preview">Preview</Segment>
		</SegmentControl>
	</div>
</div>

<style>
	.pr-header {
		display: flex;
		flex-direction: column;
		justify-content: center;
		gap: 16px;
		padding-bottom: 16px;
	}

	.pr-header__row {
		width: 100%;
		display: flex;
		align-items: center;
	}

	.pr-title {
		flex: 1;
	}
</style>
