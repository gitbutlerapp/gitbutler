<script module lang="ts">
	import HunkDiffRow from '$components/hunkDiff/HunkDiffRow.svelte';
	import LineSelection from '$components/hunkDiff/lineSelection.svelte';
	import { SectionType, encodeDiffFileLine } from '$lib/utils/diffParsing';
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import type { Row, DependencyLock } from '$lib/utils/diffParsing';

	// Mock LineSelection for stories
	const mockLineSelection = new LineSelection();

	// Sample dependency locks
	const sampleLocks: DependencyLock[] = [
		{
			stackId: 'main-stack',
			commitId: 'abc123def456'
		},
		{
			stackId: 'feature-branch',
			commitId: 'def456ghi789'
		}
	];

	// Create sample rows for different scenarios
	function createSampleRow(
		type: SectionType,
		beforeLineNumber?: number,
		afterLineNumber?: number,
		locks?: DependencyLock[]
	): Row {
		return {
			encodedLineId: encodeDiffFileLine('file.js', beforeLineNumber, afterLineNumber),
			beforeLineNumber,
			afterLineNumber,
			tokens: ['Sample', ' ', 'code', ' ', 'content'],
			type,
			size: 17,
			isLast: false,
			isDeltaLine: type !== SectionType.Context,
			locks
		};
	}

	const { Story } = defineMeta({
		title: 'Code / HunkDiffRow',
		component: HunkDiffRow,
		args: {
			idx: 0,
			tabSize: 4,
			wrapText: false,
			minWidth: 3,
			hoveringOverTable: false,
			hideCheckboxes: false,
			hunkHasLocks: false,
			lineSelection: mockLineSelection,
			row: createSampleRow(SectionType.AddedLines, undefined, 42)
		},
		argTypes: {
			idx: {
				control: { type: 'number' }
			},
			clickable: {
				control: { type: 'boolean' }
			},
			tabSize: {
				control: { type: 'number', min: 1, max: 8 }
			},
			wrapText: {
				control: { type: 'boolean' }
			},
			diffFont: {
				control: { type: 'text' }
			},
			numberHeaderWidth: {
				control: { type: 'number' }
			},
			hoveringOverTable: {
				control: { type: 'boolean' }
			},
			staged: {
				control: { type: 'boolean' }
			},
			hideCheckboxes: {
				control: { type: 'boolean' }
			},
			minWidth: {
				control: { type: 'number', min: 1, max: 10 }
			},
			hunkHasLocks: {
				control: { type: 'boolean' }
			}
		}
	});
</script>

<script lang="ts">
</script>

<Story name="Added Line - Default">
	{#snippet template(args)}
		<div class="story-wrapper">
			<table class="hunk-diff-table">
				<tbody>
					<HunkDiffRow {...args} row={createSampleRow(SectionType.AddedLines, undefined, 42)} />
				</tbody>
			</table>
		</div>
	{/snippet}
</Story>

<Story name="Removed Line">
	{#snippet template(args)}
		<div class="story-wrapper">
			<table class="hunk-diff-table">
				<tbody>
					<HunkDiffRow {...args} row={createSampleRow(SectionType.RemovedLines, 41, undefined)} />
				</tbody>
			</table>
		</div>
	{/snippet}
</Story>

<Story name="Context Line">
	{#snippet template(args)}
		<div class="story-wrapper">
			<table class="hunk-diff-table">
				<tbody>
					<HunkDiffRow {...args} row={createSampleRow(SectionType.Context, 40, 40)} />
				</tbody>
			</table>
		</div>
	{/snippet}
</Story>

<Story name="Locked Line - Single Lock">
	{#snippet template(args)}
		<div class="story-wrapper">
			<table class="hunk-diff-table">
				<tbody>
					<HunkDiffRow
						{...args}
						hunkHasLocks={true}
						row={createSampleRow(SectionType.AddedLines, undefined, 42, [sampleLocks[0]])}
					>
						{#snippet lockWarning(locks)}
							This line is locked by commit {locks[0].commitId.slice(0, 7)} on stack {locks[0]
								.stackId}
						{/snippet}
					</HunkDiffRow>
				</tbody>
			</table>
		</div>
	{/snippet}
</Story>

<Story name="Locked Line - Multiple Locks">
	{#snippet template(args)}
		<div class="story-wrapper">
			<table class="hunk-diff-table">
				<tbody>
					<HunkDiffRow
						{...args}
						hunkHasLocks={true}
						row={createSampleRow(SectionType.AddedLines, undefined, 42, sampleLocks)}
					>
						{#snippet lockWarning(locks)}
							This line is locked by multiple commits:
							{#each locks as lock, i}
								{lock.commitId.slice(0, 7)} ({lock.stackId}){i < locks.length - 1 ? ', ' : ''}
							{/each}
						{/snippet}
					</HunkDiffRow>
				</tbody>
			</table>
		</div>
	{/snippet}
</Story>

<Story name="Staged Line with Lock">
	{#snippet template(args)}
		<div class="story-wrapper">
			<table class="hunk-diff-table">
				<tbody>
					<HunkDiffRow
						{...args}
						staged={true}
						hunkHasLocks={true}
						row={createSampleRow(SectionType.AddedLines, undefined, 42, [sampleLocks[0]])}
					>
						{#snippet lockWarning(locks)}
							This staged line is locked by commit {locks[0].commitId.slice(0, 7)}
						{/snippet}
					</HunkDiffRow>
				</tbody>
			</table>
		</div>
	{/snippet}
</Story>

<Story name="Multiple Rows with Mixed Locks">
	{#snippet template(args)}
		<div class="story-wrapper">
			<table class="hunk-diff-table">
				<tbody>
					<!-- Context line -->
					<HunkDiffRow
						{...args}
						idx={0}
						hunkHasLocks={true}
						row={createSampleRow(SectionType.Context, 39, 39)}
					/>
					<!-- Locked removed line -->
					<HunkDiffRow
						{...args}
						idx={1}
						hunkHasLocks={true}
						row={createSampleRow(SectionType.RemovedLines, 40, undefined, [sampleLocks[0]])}
					>
						{#snippet lockWarning(locks)}
							Removed line locked by {locks[0].commitId.slice(0, 7)}
						{/snippet}
					</HunkDiffRow>
					<!-- Non-locked removed line (should have red lock column) -->
					<HunkDiffRow
						{...args}
						idx={2}
						hunkHasLocks={true}
						row={createSampleRow(SectionType.RemovedLines, 41, undefined)}
					/>
					<!-- Locked added line -->
					<HunkDiffRow
						{...args}
						idx={3}
						hunkHasLocks={true}
						row={createSampleRow(SectionType.AddedLines, undefined, 40, [sampleLocks[1]])}
					>
						{#snippet lockWarning(locks)}
							Added line locked by {locks[0].commitId.slice(0, 7)}
						{/snippet}
					</HunkDiffRow>
					<!-- Non-locked added line (should have green lock column) -->
					<HunkDiffRow
						{...args}
						idx={4}
						hunkHasLocks={true}
						row={createSampleRow(SectionType.AddedLines, undefined, 41)}
					/>
					<!-- Context line -->
					<HunkDiffRow
						{...args}
						idx={5}
						hunkHasLocks={true}
						row={createSampleRow(SectionType.Context, 42, 42)}
					/>
				</tbody>
			</table>
		</div>
	{/snippet}
</Story>

<Story name="Long Line with Word Wrapping">
	{#snippet template(args)}
		<div class="story-wrapper">
			<table class="hunk-diff-table">
				<tbody>
					<HunkDiffRow
						{...args}
						wrapText={true}
						hunkHasLocks={true}
						row={{
							...createSampleRow(SectionType.AddedLines, undefined, 42, [sampleLocks[0]]),
							tokens: [
								'This',
								' ',
								'is',
								' ',
								'a',
								' ',
								'very',
								' ',
								'long',
								' ',
								'line',
								' ',
								'of',
								' ',
								'code',
								' ',
								'that',
								' ',
								'should',
								' ',
								'demonstrate',
								' ',
								'word',
								' ',
								'wrapping',
								' ',
								'behavior',
								' ',
								'when',
								' ',
								'the',
								' ',
								'wrapText',
								' ',
								'prop',
								' ',
								'is',
								' ',
								'enabled',
								' ',
								'and',
								' ',
								'the',
								' ',
								'line',
								' ',
								'is',
								' ',
								'also',
								' ',
								'locked',
								' ',
								'by',
								' ',
								'a',
								' ',
								'dependency'
							],
							size: 150
						}}
					>
						{#snippet lockWarning(locks)}
							This very long line is locked by commit {locks[0].commitId}
						{/snippet}
					</HunkDiffRow>
				</tbody>
			</table>
		</div>
	{/snippet}
</Story>
