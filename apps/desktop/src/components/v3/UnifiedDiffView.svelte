<script lang="ts">
	import ReduxResult from '$components/ReduxResult.svelte';
	import HunkContextMenu from '$components/v3/HunkContextMenu.svelte';
	import LineSelection from '$components/v3/unifiedDiffLineSelection.svelte';
	import { DiffService } from '$lib/hunks/diffService.svelte';
	import { Project } from '$lib/project/project';
	import {
		ChangeSelectionService,
		type PartiallySelectedFile
	} from '$lib/selection/changeSelection.svelte';
	import { getContext, inject } from '@gitbutler/shared/context';
	import HunkDiff from '@gitbutler/ui/HunkDiff.svelte';
	import type { TreeChange } from '$lib/hunks/change';
	import type { DiffHunk } from '$lib/hunks/hunk';
	import type { LineId } from '@gitbutler/ui/utils/diffParsing';

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

	const changeSelectionResult = $derived(changeSelection.getById(change.path));
	const selection = $derived(changeSelectionResult.current);
	const pathData = $derived({
		path: change.path,
		pathBytes: change.pathBytes
	});

	const lineSelection = new LineSelection(changeSelection);

	$effect(() => {
		lineSelection.setChange(change);
	});

	$effect(() => {
		lineSelection.setSelectable(selectable);
	});

	function updateStage(hunk: DiffHunk, select: boolean, allHunks: DiffHunk[]) {
		if (selection?.type === 'full') {
			handleStageInFullSelection(select, allHunks, hunk);
			return;
		}

		if (selection?.type === 'partial') {
			handleStageInPartialSelection(selection, select, hunk, allHunks);
			return;
		}

		if (select) {
			changeSelection.add({
				type: 'partial',
				...pathData,
				hunks: [{ type: 'full', ...hunk }]
			});
			return;
		}

		throw new Error('Cannot deselect from an empty selection');
	}

	/**
	 * Handles updating the staging state of a hunk when the file it belongs to is already partially staged.
	 */
	function handleStageInPartialSelection(
		partialSelection: PartiallySelectedFile,
		select: boolean,
		hunk: DiffHunk,
		allHunks: DiffHunk[]
	) {
		const newHunks = partialSelection.hunks.slice();

		if (select) {
			newHunks.push({
				type: 'full',
				...hunk
			});

			if (newHunks.length === allHunks.length) {
				changeSelection.update({
					type: 'full',
					...pathData
				});
				return;
			}

			changeSelection.update({
				type: 'partial',
				...pathData,
				hunks: newHunks
			});

			return;
		}

		const hunks = partialSelection.hunks.filter((h) => {
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

	/**
	 * Handles updating the staging state of a hunk when the file it belongs to is already fully staged.
	 */
	function handleStageInFullSelection(select: boolean, allHunks: DiffHunk[], hunk: DiffHunk) {
		if (select) {
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
	}

	function getStageState(hunk: DiffHunk): [boolean | undefined, LineId[] | undefined] {
		if (!selectable) return [undefined, undefined];
		if (selection === undefined) return [false, undefined];
		if (selection.type === 'full') return [true, undefined];
		const hunkSelected = selection.hunks.find(
			(h) => h.newStart === hunk.newStart && h.oldStart === hunk.oldStart
		);
		const linesSelected = hunkSelected?.type === 'partial' ? hunkSelected?.lines : undefined;
		const stagedHunk = !!hunkSelected;
		return [stagedHunk, linesSelected];
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
					{@const [staged, stagedLines] = getStageState(hunk)}
					<HunkDiff
						filePath={change.path}
						hunkStr={hunk.diff}
						{staged}
						{stagedLines}
						onLineClick={(p) =>
							lineSelection.toggleStageLines(selection, hunk, p, diff.subject.hunks)}
						onChangeStage={(selected) => {
							updateStage(hunk, selected, diff.subject.hunks);
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
		background-color: var(--clr-bg-1);
		border-bottom: 1px solid var(--clr-border-2);
	}
	.hunk-content-warning {
		margin-left: 8px;
	}
</style>
