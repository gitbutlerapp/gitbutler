<script lang="ts">
	import AsyncRender from '$components/AsyncRender.svelte';
	import CommitMessageEditor from '$components/CommitMessageEditor.svelte';
	import { projectRunCommitHooks } from '$lib/config/config';
	import { HooksService } from '$lib/hooks/hooksService';
	import { type DiffSpec } from '$lib/hunks/hunk';
	import { showError, showToast } from '$lib/notifications/toasts';
	import { UncommittedService } from '$lib/selection/uncommittedService.svelte';
	import { CommitAnalytics } from '$lib/soup/commitAnalytics';
	import { StackService, type RejectionReason } from '$lib/stacks/stackService.svelte';
	import { UiState } from '$lib/state/uiState.svelte';
	import { TestId } from '$lib/testing/testIds';
	import { inject } from '@gitbutler/shared/context';
	import toasts from '@gitbutler/ui/toasts';
	import { tick } from 'svelte';

	type Props = {
		projectId: string;
		stackId?: string;
		onclose?: () => void;
	};
	const { projectId, stackId, onclose }: Props = $props();

	const [stackService, uiState, hooksService, uncommittedService, commitAnalytics] = inject(
		StackService,
		UiState,
		HooksService,
		UncommittedService,
		CommitAnalytics
	);

	const projectState = $derived(uiState.project(projectId));
	// Using a dummy stackId kind of sucks... but it's fine for now
	const stackState = $derived(uiState.stack(stackId || 'new-commit-view--new-stack'));

	const [createCommitInStack, commitCreation] = stackService.createCommit({});

	const runCommitHooks = $derived(projectRunCommitHooks(projectId));

	let isCooking = $state(false);

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

	const exclusiveAction = $derived(projectState.exclusiveAction.current);
	const commitAction = $derived(exclusiveAction?.type === 'commit' ? exclusiveAction : undefined);

	const selectedLines = $derived(uncommittedService.selectedLines(stackId));
	const topBranchResult = $derived(stackId ? stackService.branches(projectId, stackId) : undefined);
	const topBranchName = $derived(topBranchResult?.current.data?.at(0)?.name);

	const draftBranchName = $derived(uiState.global.draftBranchName.current);
	const canCommit = $derived(selectedLines.current.length > 0);

	let input = $state<ReturnType<typeof CommitMessageEditor>>();

	async function createCommit(message: string) {
		if (isCooking) {
			showToast({ message: 'Commit is already in progress', style: 'error' });
			return;
		}

		isCooking = true;
		await tick();
		try {
			let finalStackId = stackId;
			let finalBranchName = commitAction?.branchName || draftBranchName || topBranchName;
			// TODO: Refactor this awkward fallback somehow.
			if (!finalBranchName) {
				finalBranchName = await stackService.fetchNewBranchName(projectId);
			}
			const parentId = commitAction?.parentCommitId;

			if (!finalStackId) {
				const stack = await createNewStack({
					projectId,
					branch: { name: finalBranchName, order: 0 }
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

			// Get current editor mode from the component instance
			const isRichTextMode = input?.isRichTextMode?.() || false;

			// Await analytics data before creating commit
			const analyticsProperties = await commitAnalytics.getCommitProperties({
				projectId,
				stackId: finalStackId,
				selectedBranchName: finalBranchName,
				message,
				parentId,
				isRichTextMode
			});

			const response = await createCommitInStack(
				{
					projectId,
					parentId,
					stackId: finalStackId,
					message: message,
					stackBranchName: finalBranchName,
					worktreeChanges
				},
				{ properties: analyticsProperties }
			);

			const postHookFailed = await runPostHook();
			if (postHookFailed) return;

			const newId = response.newCommit;

			if (newId) {
				// Clear saved state for commit message editor.
				stackState.newCommitMessage.current = { title: '', description: '' };

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
					commitTitle: stackState.newCommitMessage.current?.title || '',
					pathsToRejectedChanges
				});
			}
		} finally {
			isCooking = false;
		}
	}

	const [createNewStack, newStackResult] = stackService.newStack;

	async function handleCommitCreation(title: string, description: string) {
		stackState.newCommitMessage.current = { title, description };

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
		let newCommitMessageUpdate: Partial<NewCommitMessage> | undefined = undefined;
		if (typeof title === 'string') {
			newCommitMessageUpdate = { title };
		}

		if (typeof description === 'string') {
			newCommitMessageUpdate = {
				...newCommitMessageUpdate,
				description
			};
		}

		if (newCommitMessageUpdate) {
			stackState.newCommitMessage.set({
				...stackState.newCommitMessage.current,
				...newCommitMessageUpdate
			});
		}
	}

	function cancel(args: { title: string; description: string }) {
		stackState.newCommitMessage.current = args;
		projectState.exclusiveAction.set(undefined);
		uncommittedService.uncheckAll(null);
		if (stackId) {
			uncommittedService.uncheckAll(stackId);
		}
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
			loading={commitCreation.current.isLoading || newStackResult.current.isLoading || isCooking}
			title={stackState.newCommitMessage.current.title}
			description={stackState.newCommitMessage.current.description}
		/>
	</div>
</AsyncRender>
