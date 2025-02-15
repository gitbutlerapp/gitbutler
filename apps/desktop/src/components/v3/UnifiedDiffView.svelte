<script lang="ts">
	import ReduxResult from '../ReduxResult.svelte';
	import { DiffService } from '$lib/hunks/diffService.svelte';
	import { ChangeSelectionService } from '$lib/selection/changeSelection.svelte';
	import { WorktreeService } from '$lib/worktree/worktreeService.svelte';
	import { inject } from '@gitbutler/shared/context';
	import HunkDiff from '@gitbutler/ui/HunkDiff.svelte';
	import type { DiffHunk } from '$lib/hunks/hunk';

	type Props = {
		path: string;
		commitId?: string;
		projectId: string;
		selectable: boolean;
	};

	const { projectId, path, selectable = false }: Props = $props();
	const [worktreeService, diffService, changeSelection] = inject(
		WorktreeService,
		DiffService,
		ChangeSelectionService
	);

	const result = $derived(
		worktreeService
			.getChange(projectId, path)
			.current.andThen((change) => diffService.getDiff(projectId, change)).current
	);

	const selection = $derived(changeSelection.getById(path).current);

	function onchange(hunk: DiffHunk, selected: boolean, allHunks: DiffHunk[]) {
		if (selection) {
			if (selection.type === 'full') {
				if (selected) {
					throw new Error('Cannot add to full selection');
				}
				const newHunks = allHunks.filter((h) => h !== hunk);
				changeSelection.update(
					path,
					newHunks.map((h) => ({
						type: 'full',
						...h
					}))
				);
			} else if (selection.type === 'partial') {
				if (selected) {
					const newHunks = selection.hunks.slice();
					newHunks.push({
						type: 'full',
						...hunk
					});
					if (newHunks.length === allHunks.length) {
						changeSelection.update(path, []);
					} else {
						changeSelection.update(path, newHunks);
					}
				} else {
					const hunks = selection.hunks.filter((h) => {
						return h.newStart !== hunk.newStart && h.newLines !== hunk.newLines;
					});
					if (hunks.length === 0) {
						changeSelection.remove(path);
					} else {
						changeSelection.update(path, hunks);
					}
				}
			}
		} else if (selected) {
			changeSelection.addPartial(path, [
				{
					type: 'full',
					...hunk
				}
			]);
		} else {
			throw new Error('Cannot deselect from an empty selection');
		}
	}
</script>

<div class="diff-section">
	<ReduxResult {result}>
		{#snippet children(diff)}
			{#if diff.type === 'Patch'}
				{#each diff.subject.hunks as hunk}
					<HunkDiff
						filePath={path}
						hunkStr={hunk.diff}
						selected={selectable
							? selection &&
								(selection.type === 'full' ||
									selection.hunks.some((h) => h.newStart === hunk.newStart))
								? true
								: false
							: undefined}
						onchange={(selected) => {
							onchange(hunk, selected, diff.subject.hunks);
						}}
					/>
				{/each}
			{/if}
		{/snippet}
	</ReduxResult>
</div>

<style lang="postcss">
	.diff-section {
		display: flex;
		padding: 14px;
		flex-direction: column;
		align-items: flex-start;
		gap: 14px;
		align-self: stretch;
		overflow-x: hidden;
		max-width: 100%;
	}
</style>
