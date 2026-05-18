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
		resolveLabel?: string;
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
		resolveLabel,
		onresolveclick,
		oncontextmenu,
	}: Props = $props();

	const conflicted = $derived(
		conflictEntryPresence !== undefined && conflictState === "conflicted" && !manuallyResolved,
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
		{resolveLabel}
		{conflictHint}
		{oncontextmenu}
	/>
</div>
