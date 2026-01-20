import { AmendCommitWithHunkDzHandler } from '$lib/commits/dropHandler';
import { HunkDropDataV3 } from '$lib/dragging/draggables';
import { describe, expect, test, vi } from 'vitest';
import type { HooksService } from '$lib/hooks/hooksService';
import type { TreeChange } from '$lib/hunks/change';
import type { HunkHeader } from '$lib/hunks/hunk';
import type { SelectionId } from '$lib/selection/key';
import type { StackService } from '$lib/stacks/stackService.svelte';
import type { UiState } from '$lib/state/uiState.svelte';

describe('AmendCommitWithHunkDzHandler', () => {
	test('uses selectedHunkHeaders when provided', async () => {
		const amendCommitMutation = vi.fn();
		const stackService = { amendCommitMutation } as unknown as StackService;
		const hooksService = {} as unknown as HooksService;
		const uiState = {} as unknown as UiState;

		const handler = new AmendCommitWithHunkDzHandler({
			stackService,
			hooksService,
			okWithForce: true,
			projectId: 'project-id',
			stackId: 'stack-id',
			commit: { id: 'commit-id', isRemote: false, isIntegrated: false, hasConflicts: false },
			uiState,
			runHooks: false
		});

		const selectionId: SelectionId = { type: 'worktree' };
		const change: TreeChange = {
			path: 'file.txt',
			pathBytes: [102, 105, 108, 101, 46, 116, 120, 116],
			status: {
				type: 'Modification',
				subject: {
					previousState: { id: 'prev', kind: 'Blob' },
					state: { id: 'next', kind: 'Blob' },
					flags: null
				}
			}
		};

		const fullHunk: HunkHeader = { oldStart: 1, oldLines: 2, newStart: 1, newLines: 3 };
		const selectedHunkHeaders: HunkHeader[] = [
			{ oldStart: 0, oldLines: 0, newStart: 2, newLines: 1 }
		];

		await handler.ondrop(
			new HunkDropDataV3(change, fullHunk, true, null, undefined, selectionId, selectedHunkHeaders)
		);

		expect(amendCommitMutation).toHaveBeenCalledTimes(1);
		expect(amendCommitMutation.mock.calls[0]![0]!.worktreeChanges[0]!.hunkHeaders).toEqual(
			selectedHunkHeaders
		);
	});

	test.each([
		{ name: 'undefined', selectedHunkHeaders: undefined },
		{ name: 'empty array', selectedHunkHeaders: [] as HunkHeader[] }
	])(
		'falls back to full hunk when selectedHunkHeaders is $name',
		async ({ selectedHunkHeaders }) => {
			const amendCommitMutation = vi.fn();
			const stackService = { amendCommitMutation } as unknown as StackService;
			const hooksService = {} as unknown as HooksService;
			const uiState = {} as unknown as UiState;

			const handler = new AmendCommitWithHunkDzHandler({
				stackService,
				hooksService,
				okWithForce: true,
				projectId: 'project-id',
				stackId: 'stack-id',
				commit: { id: 'commit-id', isRemote: false, isIntegrated: false, hasConflicts: false },
				uiState,
				runHooks: false
			});

			const selectionId: SelectionId = { type: 'worktree' };
			const change: TreeChange = {
				path: 'file.txt',
				pathBytes: [102, 105, 108, 101, 46, 116, 120, 116],
				status: {
					type: 'Modification',
					subject: {
						previousState: { id: 'prev', kind: 'Blob' },
						state: { id: 'next', kind: 'Blob' },
						flags: null
					}
				}
			};

			const fullHunk: HunkHeader = { oldStart: 1, oldLines: 2, newStart: 1, newLines: 3 };

			await handler.ondrop(
				new HunkDropDataV3(
					change,
					fullHunk,
					true,
					null,
					undefined,
					selectionId,
					selectedHunkHeaders
				)
			);

			expect(amendCommitMutation).toHaveBeenCalledTimes(1);
			expect(amendCommitMutation.mock.calls[0]![0]!.worktreeChanges[0]!.hunkHeaders).toEqual([
				fullHunk
			]);
		}
	);
});
