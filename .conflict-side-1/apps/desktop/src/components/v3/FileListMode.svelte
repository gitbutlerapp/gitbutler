<script lang="ts">
	import { persisted } from '@gitbutler/shared/persisted';
	import Segment from '@gitbutler/ui/segmentControl/Segment.svelte';
	import SegmentControl from '@gitbutler/ui/segmentControl/SegmentControl.svelte';

	type Mode = 'tree' | 'list';
	type Props = {
		mode: Mode;
		persist: 'uncommitted' | 'committed';
	};

	let { persist, mode = $bindable() }: Props = $props();

	let saved = persisted<Mode | undefined>(undefined, persist);

	$effect(() => {
		if ($saved) {
			mode = $saved;
		}
	});
</script>

<SegmentControl
	defaultIndex={$saved === 'list' ? 0 : 1}
	onselect={(id) => {
		// TODO: Refactor SegmentControl.
		$saved = id as Mode;
	}}
	size="small"
>
	<Segment id="list" icon="list-view" />
	<Segment id="tree" icon="tree-view" />
</SegmentControl>
