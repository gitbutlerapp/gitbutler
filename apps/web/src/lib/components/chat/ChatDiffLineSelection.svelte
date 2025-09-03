<script lang="ts">
	import { type DiffLineSelected, type DiffSelection } from '$lib/diff/lineSelection.svelte';

	import { Button, FileIcon } from '@gitbutler/ui';
	import { encodeDiffLineRange } from '@gitbutler/ui/utils/diffParsing';

	interface Props {
		diffSelection: DiffSelection;
		clearDiffSelection: () => void;
	}

	const { diffSelection, clearDiffSelection }: Props = $props();

	function getLineSelectionEncoding(lines: DiffLineSelected[]) {
		const sortedLines = lines.sort((a, b) => a.index - b.index);
		return encodeDiffLineRange(sortedLines);
	}

	const selectionLabel = $derived(
		`${diffSelection.fileName}:${getLineSelectionEncoding(diffSelection.lines)}`
	);
</script>

<div class="diff-selection">
	<div class="diff-selection__content">
		<FileIcon fileName={diffSelection.fileName} size={16} />
		<p class="text-12 text-body file-name">
			{selectionLabel}
		</p>
	</div>

	<Button icon="cross" style="neutral" kind="ghost" size="tag" onclick={clearDiffSelection} />
</div>

<style>
	.diff-selection {
		display: flex;
		align-items: center;
		align-self: stretch;
		justify-content: space-between;
		margin: 6px 6px 0;
		padding: 6px;
		gap: 8px;
		border: 1px solid var(--clr-border-2);

		border-radius: var(--radius-m);
		background: var(--clr-bg-1);
	}

	.diff-selection__content {
		display: flex;
		align-items: center;
		gap: 8px;
	}
</style>
