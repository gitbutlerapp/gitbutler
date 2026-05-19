<script lang="ts">
	import CommitMessageEditor from "$components/commit/CommitMessageEditor.svelte";
	import { COMMIT_ANALYTICS } from "$lib/analytics/commitAnalytics";
	import { projectRunCommitHooks } from "$lib/config/config";
	import { showError } from "$lib/error/showError";
	import { HookFailedError, HOOKS_SERVICE } from "$lib/git/hooksService";
	import { showToast } from "$lib/notifications/toasts";
	import { FILE_SELECTION_MANAGER } from "$lib/selection/fileSelectionManager.svelte";
	import { createWorktreeSelection } from "$lib/selection/key";
	import { UNCOMMITTED_SERVICE } from "$lib/selection/uncommittedService.svelte";
	import { hasOnlyNoEffectiveChanges } from "$lib/stacks/stackEndpoints";
	import { STACK_SERVICE } from "$lib/stacks/stackService.svelte";
	import { UI_STATE, type NewCommitMessage, type RejectionReason } from "$lib/state/uiState.svelte";
	import { inject } from "@gitbutler/core/context";
	import { TestId } from "@gitbutler/ui";
	import { tick } from "svelte";
	import type { EventProperties } from "$lib/state/customHooks.svelte";

	type Props = {
		projectId: string;
		stackId?: string;
		onclose?: () => void;
	};
	const { projectId, stackId, onclose }: Props = $props();

	const stackService = inject(STACK_SERVICE);
	const uiState = inject(UI_STATE);
	const hooksService = inject(HOOKS_SERVICE);
	const uncommittedService = inject(UNCOMMITTED_SERVICE);
	const commitAnalytics = inject(COMMIT_ANALYTICS);
	const idSelection = inject(FILE_SELECTION_MANAGER);

	const projectState = $derived(uiState.project(projectId));
	// Using a dummy stackId kind of sucks... but it's fine for now
	const laneState = $derived(uiState.lane(stackId || "new-commit-view--new-stack"));

	const [createCommitInStack, commitCreation] = stackService.createCommit();
	const [createNewStack, newStackQuery] = stackService.newStack;
	const [runMessageHook] = hooksService.message;

	let isCooking = $state(false);
	let commitStatus = $state<string | undefined>();

	const exclusiveAction = $derived(projectState.exclusiveAction.current);
	const commitAction = $derived(exclusiveAction?.type === "commit" ? exclusiveAction : undefined);
	const isLoading = $derived(
		commitCreation.current.isLoading || newStackQuery.current.isLoading || isCooking,
	);

	const selectedLines = $derived(uncommittedService.selectedLines(stackId));
	const topBranchQuery = $derived(stackId ? stackService.branches(projectId, stackId) : undefined);
	const topBranchName = $derived(topBranchQuery?.response?.at(0)?.name);

	const draftBranchName = $derived(uiState.global.draftBranchName.current);
	const canCommit = $derived(selectedLines.current.length > 0);

	let input = $state<ReturnType<typeof CommitMessageEditor>>();
	const runCommitHooks = $derived(projectRunCommitHooks(projectId));

	async function getCommitAnalyticsProperties(args: {
		projectId: string;
		stackId: string;
		selectedBranchName: string;
		message: string;
		parentId?: string;
		isRichTextMode?: boolean;
	}): Promise<EventProperties> {
		let timedOut = false;
		const timeout = new Promise<EventProperties>((resolve) => {
			setTimeout(() => {
				timedOut = true;
				resolve({});
			}, 250);
		});

		const properties = await Promise.race([commitAnalytics.getCommitProperties(args), timeout]);
		if (timedOut) {
			console.warn("Commit analytics took too long; creating commit without analytics properties.");
		}
		return properties;
	}

	async function createCommit(message: string) {
		if (isCooking) {
			showToast({ message: "Commit is already in progress", style: "danger" });
			return;
		}

		isCooking = true;
		commitStatus = "Preparing commit...";
		await tick();
		try {
			let finalStackId = stackId;
			let finalBranchName = commitAction?.branchName || draftBranchName || topBranchName;
			// TODO: Refactor this awkward fallback somehow.
			if (!finalBranchName) {
				finalBranchName = await stackService.fetchNewBranchName(projectId);
			}
			const parentId = commitAction?.parentCommitId;
			const insertBelow = commitAction?.insertBelow;

			if (!finalStackId) {
				const stack = await createNewStack({
					projectId,
					branch: { name: finalBranchName, order: 0 },
				});
				finalStackId = stack.id ?? undefined;
				finalBranchName = stack.heads[0]?.name; // Updated to access the name property
				uiState.global.draftBranchName.set(undefined);
			}

			if (!finalStackId) {
				throw new Error("No stack selected!");
			}

			if (!finalBranchName) {
				throw new Error("No branch selected!");
			}

			// Run commit-msg hook if hooks are enabled
			let finalMessage = message;
			if ($runCommitHooks) {
				commitStatus = "Running commit message hook...";
				const messageHookResult = await runMessageHook({ projectId, message });
				if (messageHookResult?.status === "failure") {
					showError("Commit message hook failed", messageHookResult.error);
					return;
				} else if (messageHookResult?.status === "message") {
					finalMessage = messageHookResult.message;
				}
			}

			commitStatus = "Preparing selected changes...";
			const worktreeChanges = await uncommittedService.worktreeChanges(projectId, stackId);
			if (worktreeChanges.length === 0) {
				showToast({
					style: "info",
					title: "No changes to commit",
					message: "The selected changes are no longer available. Refresh or reselect changes to commit.",
				});
				uncommittedService.clearHunkSelection();
				idSelection.clear(createWorktreeSelection({ stackId }));
				return;
			}

			// Get current editor mode from the component instance
			const isRichTextMode = input?.isRichTextMode?.() || false;

			commitStatus = "Preparing commit metadata...";
			const analyticsProperties = await getCommitAnalyticsProperties({
				projectId,
				stackId: finalStackId,
				selectedBranchName: finalBranchName,
				message: finalMessage,
				parentId,
				isRichTextMode,
			});

			if ($runCommitHooks) {
				commitStatus = "Running pre-commit hooks...";
				await hooksService.runPreCommitHooks(projectId, worktreeChanges);
			}

			commitStatus = "Creating commit...";
			const response = await createCommitInStack(
				{
					projectId,
					parentId,
					insertBelow,
					stackId: finalStackId,
					message: finalMessage,
					stackBranchName: finalBranchName,
					worktreeChanges,
					dryRun: false,
				},
				{ properties: analyticsProperties },
			);

			if ($runCommitHooks) {
				commitStatus = "Running post-commit hooks...";
				await hooksService.runPostCommitHooks(projectId);
			}

			commitStatus = "Finishing commit...";
			const newId = response.newCommit;

			if (newId) {
				// Clear saved state for commit message editor.
				laneState.newCommitMessage.set({ title: "", description: "" });

				// Close the drawer.
				projectState.exclusiveAction.set(undefined);

				// Clear change/hunk selection used for creating the commit.
				uncommittedService.clearHunkSelection();
				idSelection.clear(createWorktreeSelection({ stackId }));
			}

			if (hasOnlyNoEffectiveChanges(response)) {
				showToast({
					style: "info",
					title: "No changes to commit",
					message:
						"Those selected changes no longer change the target branch. Refresh or reselect changes to commit.",
				});
				uncommittedService.clearHunkSelection();
				idSelection.clear(createWorktreeSelection({ stackId }));
				return;
			}

			if (response.rejectedChanges.length > 0) {
				const pathsToRejectedChanges = response.rejectedChanges.reduce(
					(acc: Record<string, RejectionReason>, { reason, path }) => {
						acc[path] = reason;
						return acc;
					},
					{},
				);

				uiState.global.modal.set({
					type: "commit-failed",
					projectId,
					targetBranchName: finalBranchName,
					newCommitId: newId ?? undefined,
					commitTitle: laneState.newCommitMessage.current?.title || "",
					pathsToRejectedChanges,
				});
			}
		} finally {
			isCooking = false;
			commitStatus = undefined;
		}
	}

	async function handleCommitCreation(title: string, description: string) {
		laneState.newCommitMessage.set({ title, description });

		const message = description ? title + "\n\n" + description : title;
		if (!message) {
			showToast({ message: "Commit message is required", style: "danger" });
			return;
		}

		try {
			await createCommit(message);
		} catch (err: unknown) {
			if (!(err instanceof HookFailedError)) {
				showError("Failed to commit", err);
			}
		}
	}

	function handleMessageUpdate(title?: string, description?: string) {
		let newCommitMessageUpdate: Partial<NewCommitMessage> | undefined = undefined;
		if (typeof title === "string") {
			newCommitMessageUpdate = { title };
		}

		if (typeof description === "string") {
			newCommitMessageUpdate = {
				...newCommitMessageUpdate,
				description,
			};
		}

		if (newCommitMessageUpdate) {
			laneState.newCommitMessage.set({
				...laneState.newCommitMessage.current,
				...newCommitMessageUpdate,
			});
		}
	}

	function cancel(args: { title: string; description: string }) {
		laneState.newCommitMessage.set(args);
		projectState.exclusiveAction.set(undefined);
		uncommittedService.uncheckAll(null);
		if (stackId) {
			uncommittedService.uncheckAll(stackId);
		}
		onclose?.();
	}
</script>

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
		loading={isLoading}
		loadingLabel={commitStatus}
		title={laneState.newCommitMessage.current.title}
		description={laneState.newCommitMessage.current.description}
	/>
</div>
