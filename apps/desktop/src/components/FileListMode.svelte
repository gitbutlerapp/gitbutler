<script lang="ts">
	import { SETTINGS } from '$lib/settings/userSettings';
	import { inject } from '@gitbutler/core/context';
	import { persisted } from '@gitbutler/shared/persisted';
	import { Segment, SegmentControl, TestId } from '@gitbutler/ui';

	type Mode = 'tree' | 'list';
	type Props = {
		mode: Mode;
		persistId: string;
	};

	let { persistId, mode = $bindable() }: Props = $props();

	const userSettings = inject(SETTINGS);
	let saved = persisted<Mode | undefined>(undefined, `file-list-mode-${persistId}`);

	// Initialize mode from saved value or default setting (runs once on mount)
	$effect(() => {
		mode = $saved ?? $userSettings.defaultFileListMode;
	});
</script>

<SegmentControl
	defaultIndex={mode === 'list' ? 0 : 1}
	onselect={(id) => {
		// Update saved preference; the effect will sync mode
		$saved = id as Mode;
	}}
	size="small"
>
	<Segment id="list" testId={TestId.FileListModeOption} icon="list-view" />
	<Segment id="tree" testId={TestId.FileListModeOption} icon="tree-view" />
</SegmentControl>
