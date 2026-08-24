<script lang="ts">
	import { FileListItem } from "@gitbutler/ui";
	import type { ConflictState } from "$lib/files/conflictEntryPresence";
	import type { ConflictEntryPresence } from "@gitbutler/but-sdk";
	import type { FileStatus } from "@gitbutler/ui/components/file/types";

	type Props = {
		filePath: string;
		pathFirst: boolean;
		fileStatus?: FileStatus;
		conflictHint?: string;
		conflictEntryPresence?: ConflictEntryPresence;
		conflictState: ConflictState;
		manuallyResolved: boolean;
		onresolveclick?: () => void;
		oncontextmenu?: (e: MouseEvent) => void;
	};

	const {
		filePath,
		pathFirst,
		fileStatus,
		conflictHint,
		conflictEntryPresence,
		conflictState,
		manuallyResolved,
		onresolveclick,
		oncontextmenu,
	}: Props = $props();

	// "unknown" counts as conflicted: better to offer "Mark resolved" for a
	// file we cannot judge than to falsely present it as resolved.
	const conflicted = $derived(
		conflictEntryPresence !== undefined && conflictState !== "resolved" && !manuallyResolved,
	);
</script>

<div class="file">
	<FileListItem
		{filePath}
		{pathFirst}
		{fileStatus}
		{conflicted}
		clickable={false}
		{onresolveclick}
		{conflictHint}
		{oncontextmenu}
	/>
</div>
