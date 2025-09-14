<script lang="ts">
	import { persisted } from '@gitbutler/shared/persisted';
	import { Segment, SegmentControl, TestId } from '@gitbutler/ui';

	type Mode = 'tree' | 'list';
	type Props = {
		mode: Mode;
		persist: 'uncommitted' | 'committed';
	};

	let { persist, mode = $bindable() }: Props = $props();

	let saved = persisted<Mode | undefined>(undefined, persist);

	$effect(() => {
		if ($saved !== undefined && $saved !== mode) {
			mode = $saved;
		}
	});
</script>

<SegmentControl
	defaultIndex={mode === 'list' ? 0 : 1}
	onselect={(id) => {
		// TODO: Refactor SegmentControl.
		$saved = id as Mode;
	}}
	size="small"
>
	<Segment id="list" testId={TestId.FileListModeOption} icon="list-view" />
	<Segment id="tree" testId={TestId.FileListModeOption} icon="tree-view" />
</SegmentControl>
