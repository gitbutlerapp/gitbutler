<script lang="ts">
	import HunkContextMenu from './HunkContextMenu.svelte';
	import ReduxResult from '../ReduxResult.svelte';
	import { DiffService } from '$lib/hunks/diffService.svelte';
	import { Project } from '$lib/project/project';
	import { ChangeSelectionService } from '$lib/selection/changeSelection.svelte';
	import { getContext, inject } from '@gitbutler/shared/context';
	import HunkDiff from '@gitbutler/ui/HunkDiff.svelte';
	import type { TreeChange } from '$lib/hunks/change';
	import type { DiffHunk } from '$lib/hunks/hunk';

	type Props = {
		projectId: string;
		selectable: boolean;
		change: TreeChange;
	};

	const { projectId, selectable = false, change }: Props = $props();
	const project = getContext(Project);
	let contextMenu = $state<ReturnType<typeof HunkContextMenu>>();
	let viewport = $state<HTMLDivElement>();

	const [diffService, changeSelection] = inject(DiffService, ChangeSelectionService);
	const diffResult = $derived(diffService.getDiff(projectId, change));

	const selection = $derived(changeSelection.getById(change.path).current);
	const pathData = $derived({
		path: change.path,
		pathBytes: change!.pathBytes
	});

	function stage(hunk: DiffHunk, selected: boolean, allHunks: DiffHunk[]) {
		if (selection) {
			if (selection.type === 'full') {
				if (selected) {
					throw new Error('Cannot add to full selection');
				}
				const newHunks = allHunks.filter((h) => h !== hunk);
				changeSelection.update({
					...pathData,
					type: 'partial',
					hunks: newHunks.map((h) => ({
						type: 'full',
						...h
					}))
				});
			} else if (selection.type === 'partial') {
				if (selected) {
					const newHunks = selection.hunks.slice();
					newHunks.push({
						type: 'full',
						...hunk
					});
					if (newHunks.length === allHunks.length) {
						changeSelection.update({
							type: 'full',
							...pathData
						});
					} else {
						changeSelection.update({
							type: 'partial',
							...pathData,
							hunks: newHunks
						});
					}
				} else {
					const hunks = selection.hunks.filter((h) => {
						return h.newStart !== hunk.newStart && h.newLines !== hunk.newLines;
					});
					if (hunks.length === 0) {
						changeSelection.remove(change.path);
					} else {
						changeSelection.update({
							type: 'partial',
							...pathData,
							hunks
						});
					}
				}
			}
		} else if (selected) {
			changeSelection.add({
				type: 'partial',
				...pathData,
				hunks: [{ type: 'full', ...hunk }]
			});
		} else {
			throw new Error('Cannot deselect from an empty selection');
		}
	}
</script>

<HunkContextMenu
	bind:this={contextMenu}
	trigger={viewport}
	projectPath={project.vscodePath}
	filePath={change.path}
	readonly={false}
/>

<div class="diff-section" bind:this={viewport}>
	<ReduxResult result={diffResult.current}>
		{#snippet children(diff)}
			{#if diff.type === 'Patch'}
				{#each diff.subject.hunks as hunk}
					<HunkDiff
						filePath={change.path}
						hunkStr={hunk.diff}
						staged={selectable
							? selection &&
								(selection.type === 'full' ||
									selection.hunks.some((h) => h.newStart === hunk.newStart))
								? true
								: false
							: undefined}
						onChangeStage={(selected) => {
							stage(hunk, selected, diff.subject.hunks);
						}}
						handleLineContextMenu={(params) => {
							contextMenu?.open(params.event, {
								hunk,
								beforeLineNumber: params.beforeLineNumber,
								afterLineNumber: params.afterLineNumber
							});
						}}
					/>
				{:else}
					<span class="text-14 hunk-content-warning">No content</span>
				{/each}
			{:else if diff.type === 'TooLarge'}
				Too large!
			{:else if diff.type === 'Binary'}
				Binary!
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
	.hunk-content-warning {
		margin-left: 8px;
	}
</style>
