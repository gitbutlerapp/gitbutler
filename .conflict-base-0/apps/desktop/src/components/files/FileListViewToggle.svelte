<script lang="ts">
	import { UI_STATE } from "$lib/state/uiState.svelte";
	import { inject } from "@gitbutler/core/context";
	import { persisted } from "@gitbutler/shared/persisted";
	import { SegmentControl, TestId } from "@gitbutler/ui";

	type Mode = "tree" | "list";
	type Props = {
		mode: Mode;
		persistId: string;
	};

	let { persistId, mode = $bindable() }: Props = $props();

	const uiState = inject(UI_STATE);
	const saved = $derived(persisted<Mode | undefined>(undefined, `file-list-mode-${persistId}`));

	// Subscribe to the saved store and update mode
	$effect(() => {
		return saved.subscribe((value) => {
			mode = value ?? uiState.global.defaultFileListMode.current;
		});
	});
</script>

<SegmentControl
	selected={mode}
	onselect={(id) => {
		// Update saved preference; the effect will sync mode
		saved.set(id as Mode);
	}}
	size="small"
>
	<SegmentControl.Item id="list" testId={TestId.FileListModeOption} icon="list" />
	<SegmentControl.Item id="tree" testId={TestId.FileListModeOption} icon="folder-tree" />
</SegmentControl>
