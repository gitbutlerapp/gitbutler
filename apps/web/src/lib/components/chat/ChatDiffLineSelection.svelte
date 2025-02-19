<script lang="ts">
	import { encodeLineSelection, type DiffSelection } from '$lib/diff/lineSelection.svelte';
	import Button from '@gitbutler/ui/Button.svelte';
	import FileIcon from '@gitbutler/ui/file/FileIcon.svelte';

	interface Props {
		diffSelection: DiffSelection;
		clearDiffSelection: () => void;
	}

	const { diffSelection, clearDiffSelection }: Props = $props();

	const selectionLabel = $derived(
		`${diffSelection.fileName}:${encodeLineSelection(diffSelection.lines)}`
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
		justify-content: space-between;
		padding: 6px;
		margin: 6px 6px 0;
		gap: 8px;
		align-self: stretch;

		border-radius: var(--radius-m);
		border: 1px solid var(--clr-border-2);
		background: var(--clr-bg-1);
	}

	.diff-selection__content {
		display: flex;
		align-items: center;
		gap: 8px;
	}
</style>
