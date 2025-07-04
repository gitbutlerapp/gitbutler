<script lang="ts">
	import AsyncRender from '$components/v3/AsyncRender.svelte';
	import CommitMessageEditor from '$components/v3/CommitMessageEditor.svelte';
	import { projectRunCommitHooks } from '$lib/config/config';
	import { HooksService } from '$lib/hooks/hooksService';
	import { type DiffSpec } from '$lib/hunks/hunk';
	import { showError, showToast } from '$lib/notifications/toasts';
	import { UncommittedService } from '$lib/selection/uncommittedService.svelte';
	import { StackService, type RejectionReason } from '$lib/stacks/stackService.svelte';
	import { UiState } from '$lib/state/uiState.svelte';
	import { TestId } from '$lib/testing/testIds';
	import { inject } from '@gitbutler/shared/context';
	import toasts from '@gitbutler/ui/toasts';

	type Props = {
		projectId: string;
		stackId?: string;
		onclose?: () => void;
	};
	const { projectId, stackId, onclose }: Props = $props();

	const [stackService, uiState, hooksService, uncommittedService] = inject(
		StackService,
		UiState,
		HooksService,
		UncommittedService
	);

	const projectState = $derived(uiState.project(projectId));

	const useFloatingCommitBox = $derived(uiState.global.useFloatingCommitBox);

	const [createCommitInStack, commitCreation] = stackService.createCommit({
		propertiesFn: () => ({
			floatingCommitBox: useFloatingCommitBox.current
		})
	});

	const runCommitHooks = $derived(projectRunCommitHooks(projectId));

	async function runPreHook(changes: DiffSpec[]): Promise<boolean> {
		if (!$runCommitHooks) return false;

		let failed = false;
		await toasts.promise(
			(async () => {
				const result = await hooksService.preCommitDiffspecs(projectId, changes);
				if (result?.status === 'failure') {
					failed = true;
					throw new Error(result.error);
				}
			})(),
			{
				loading: 'Started pre-commit hooks',
				success: 'Pre-commit hooks succeded',
				error: (error: Error) => `Post-commit hooks failed: ${error.message}`
			}
		);

		return failed;
	}

	async function runPostHook(): Promise<boolean> {
		if (!$runCommitHooks) return false;

		let failed = false;
		await toasts.promise(
			(async () => {
				const result = await hooksService.postCommit(projectId);
				if (result?.status === 'failure') {
					failed = true;
					throw new Error(result.error);
				}
			})(),
			{
				loading: 'Started pre-commit hooks',
				success: 'Pre-commit hooks succeded',
				error: (error: Error) => `Post-commit hooks failed: ${error.message}`
			}
		);

		return failed;
	}

	const stackState = $derived(stackId ? uiState.stack(stackId) : undefined);
	const selection = $derived(stackState?.selection.current);

	const exclusiveAction = $derived(projectState.exclusiveAction.current);
	const commitAction = $derived(exclusiveAction?.type === 'commit' ? exclusiveAction : undefined);

	const selectedLines = $derived(uncommittedService.selectedLines(stackId));
	const topBranchResult = $derived(stackId ? stackService.branches(projectId, stackId) : undefined);
	const topBranchName = $derived(topBranchResult?.current.data?.at(0)?.name);

	const draftBranchName = $derived(uiState.global.draftBranchName.current);

	const selectedBranchName = $derived(selection?.branchName || topBranchName);
	const canCommit = $derived(
		(selectedBranchName || draftBranchName || topBranchName) && selectedLines.current.length > 0
	);

	let input = $state<ReturnType<typeof CommitMessageEditor>>();

	async function createCommit(message: string) {
		let finalStackId = stackId;
		let finalBranchName = commitAction?.branchName || topBranchName;
		const parentId = commitAction?.parentCommitId;

		if (!finalStackId) {
			const stack = await createNewStack({
				projectId,
				branch: { name: draftBranchName, order: 0 }
			});
			finalStackId = stack.id;
			projectState.stackId.set(finalStackId);
			finalBranchName = stack.heads[0]?.name; // Updated to access the name property
			uiState.global.draftBranchName.set(undefined);
		}

		if (!finalStackId) {
			throw new Error('No stack selected!');
		}

		if (!finalBranchName) {
			throw new Error('No branch selected!');
		}

		const worktreeChanges = await uncommittedService.worktreeChanges(projectId, stackId);

		const preHookFailed = await runPreHook(worktreeChanges);
		if (preHookFailed) return;

		const response = await createCommitInStack(
			{
				projectId,
				parentId,
				stackId: finalStackId,
				message: message,
				stackBranchName: finalBranchName,
				worktreeChanges
			},
			{ properties: { messageLength: message.length } }
		);

		const postHookFailed = await runPostHook();
		if (postHookFailed) return;

		const newId = response.newCommit;

		if (newId) {
			// Clear saved state for commit message editor.
			projectState.commitTitle.set('');
			projectState.commitDescription.set('');

			// Close the drawer.
			projectState.exclusiveAction.set(undefined);

			// Clear change/hunk selection used for creating the commit.
			uncommittedService.clearHunkSelection();
		}

		if (response.pathsToRejectedChanges.length > 0) {
			const pathsToRejectedChanges = response.pathsToRejectedChanges.reduce(
				(acc: Record<string, RejectionReason>, [reason, path]) => {
					acc[path] = reason;
					return acc;
				},
				{}
			);

			uiState.global.modal.set({
				type: 'commit-failed',
				projectId,
				targetBranchName: finalBranchName,
				newCommitId: newId ?? undefined,
				commitTitle: projectState.commitTitle.current,
				pathsToRejectedChanges
			});
		}
	}

	const [createNewStack, newStackResult] = stackService.newStack;

	async function handleCommitCreation(title: string, description: string) {
		projectState.commitTitle.set(title);
		projectState.commitDescription.set(description);

		const message = description ? title + '\n\n' + description : title;
		if (!message) {
			showToast({ message: 'Commit message is required', style: 'error' });
			return;
		}

		try {
			await createCommit(message);
		} catch (err: unknown) {
			showError('Failed to commit', err);
		}
	}

	function handleMessageUpdate(title?: string, description?: string) {
		if (typeof title === 'string') {
			projectState.commitTitle.set(title);
		}
		if (typeof description === 'string') {
			projectState.commitDescription.set(description);
		}
	}

	function cancel(args: { title: string; description: string }) {
		projectState.commitTitle.set(args.title);
		projectState.commitDescription.set(args.description);
		projectState.exclusiveAction.set(undefined);
		uncommittedService.uncheckAll(null);
		onclose?.();
	}
</script>

<AsyncRender>
	<div data-testid={TestId.NewCommitView}>
		<CommitMessageEditor
			bind:this={input}
			{projectId}
			{stackId}
			actionLabel="Create commit"
			action={({ title, description }) => handleCommitCreation(title, description)}
			onChange={({ title, description }) => handleMessageUpdate(title, description)}
			onCancel={cancel}
			disabledAction={!canCommit}
			loading={commitCreation.current.isLoading || newStackResult.current.isLoading}
			title={projectState.commitTitle.current}
			description={projectState.commitDescription.current}
		/>
	</div>
</AsyncRender>
