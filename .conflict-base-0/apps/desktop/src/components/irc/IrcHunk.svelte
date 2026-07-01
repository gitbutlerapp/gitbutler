<script lang="ts">
	import { UI_STATE } from "$lib/state/uiState.svelte";
	import { inject } from "@gitbutler/core/context";
	import { HunkDiff, FileIcon } from "@gitbutler/ui";
	import type { DiffHunk } from "@gitbutler/but-sdk";
	import type { TreeChange } from "@gitbutler/but-sdk";

	type Props = {
		change: TreeChange;
		diff: DiffHunk;
		onLineContextMenu?: (params: {
			filePath: string;
			lineNumber: number;
			event?: MouseEvent;
			target?: HTMLElement;
		}) => void;
	};

	const { change, diff, onLineContextMenu }: Props = $props();

	const uiState = inject(UI_STATE);

	const parts = $derived(change.path.split("/"));
	const fileName = $derived(parts.at(-1) ?? "");
	const dirPath = $derived(parts.slice(0, -1).join("/"));
</script>

<div class="irc-hunk">
	<div class="irc-hunk-file">
		<FileIcon {fileName} />
		{#if dirPath}
			<span class="file-path text-12">{dirPath}/</span>
		{/if}
		<span class="file-name text-12 text-semibold">{fileName}</span>
	</div>
	<div class="irc-hunk-diff">
		<HunkDiff
			draggingDisabled={true}
			hideCheckboxes={true}
			filePath={change.path}
			hunkStr={diff.diff}
			{...uiState.pick(
				"diffLigatures",
				"tabSize",
				"wrapText",
				"diffFont",
				"diffFontSize",
				"strongContrast",
				"colorBlindFriendly",
				"inlineUnifiedDiffs",
			)}
			handleLineContextMenu={(params) => {
				onLineContextMenu?.({
					filePath: change.path,
					lineNumber: params.afterLineNumber ?? params.beforeLineNumber ?? diff.newStart,
					event: params.event,
					target: params.target,
				});
			}}
		/>
	</div>
</div>

<style lang="postcss">
	.irc-hunk {
		display: flex;
		flex-direction: column;
		margin-bottom: 6px;
		gap: 4px;
	}
	.irc-hunk-file {
		display: flex;
		align-items: center;
		padding: 4px 8px;
		gap: 6px;
	}
	.file-path {
		overflow: hidden;
		color: var(--text-3);
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.file-name {
		color: var(--text-1);
		white-space: nowrap;
	}
	.irc-hunk-diff {
		overflow-y: auto;
		white-space: initial;
	}
</style>
