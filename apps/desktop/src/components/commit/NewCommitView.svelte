<!--
	The commit box of a stack, of a new stack, or of a linked worktree's lane.

	Hooks and analytics are left out for a worktree, since they work on the
	main checkout and its stacks.
-->
<script lang="ts">
	import CommitMessageEditor from "$components/commit/CommitMessageEditor.svelte";
	import { COMMIT_ANALYTICS } from "$lib/analytics/commitAnalytics";
	import { projectRunCommitHooks } from "$lib/config/config";
	import { HOOKS_SERVICE } from "$lib/git/hooksService";
	import { showToast, showWarning } from "$lib/notifications/toasts";
	import { FILE_SELECTION_MANAGER } from "$lib/selection/fileSelectionManager.svelte";
	import { createWorktreeSelection } from "$lib/selection/key";
	import { UNCOMMITTED_SERVICE } from "$lib/selection/uncommittedService.svelte";
	import {
		rejectedChangesByPath,
		toCommitCreatePlacement,
		type CommitPlacement,
	} from "$lib/stacks/stackEndpoints";
	import { STACK_SERVICE } from "$lib/stacks/stackService.svelte";
	import { UI_STATE, type NewCommitMessage } from "$lib/state/uiState.svelte";
	import { inject } from "@gitbutler/core/context";
	import { TestId } from "@gitbutler/ui-svelte";
	import { tick } from "svelte";
	import type { Worktree } from "@gitbutler/but-sdk";

	type Props = {
		projectId: string;
		stackId?: string;
		/** Commit this linked worktree's files instead of the main checkout's. */
		worktree?: Worktree;
		/** The lane whose saved commit message this edits. */
		laneId?: string;
	};
	const { projectId, stackId, worktree, laneId = "new-commit-view--new-stack" }: Props = $props();

	/** Where a commit lands, and the branch it goes on. */
	type CommitPlan = CommitPlacement & {
		branchName: string;
		/** Set for a stack commit, which reports analytics. */
		stackId?: string;
	};

	const stackService = inject(STACK_SERVICE);
	const uiState = inject(UI_STATE);
	const hooksService = inject(HOOKS_SERVICE);
	const uncommittedServices = inject(UNCOMMITTED_SERVICE);
	const commitAnalytics = inject(COMMIT_ANALYTICS);
	const idSelection = inject(FILE_SELECTION_MANAGER);

	const uncommittedService = $derived(uncommittedServices.at(worktree?.name));
	const projectState = $derived(uiState.project(projectId));
	const laneState = $derived(uiState.lane(laneId));

	const [createCommitMutation, commitCreation] = stackService.createCommit();
	const [createNewStack, newStackQuery] = stackService.newStack;
	const [runMessageHook] = hooksService.message;

	let isCooking = $state(false);

	const exclusiveAction = $derived(projectState.exclusiveAction.current);
	const commitAction = $derived(exclusiveAction?.type === "commit" ? exclusiveAction : undefined);

	const selectedLines = $derived(uncommittedService.selectedLines(stackId));
	const topBranchQuery = $derived(stackId ? stackService.branches(projectId, stackId) : undefined);
	const topBranchName = $derived(topBranchQuery?.response?.at(0)?.refName?.displayName);
	const draftBranchName = $derived(uiState.global.draftBranchName.current);

	let input = $state<ReturnType<typeof CommitMessageEditor>>();
	const runCommitHooks = $derived(projectRunCommitHooks(projectId));

	/** A worktree commits onto its branch, or onto its detached `HEAD`. */
	function worktreePlan(worktree: Worktree): CommitPlan {
		const branchName = worktree.refName?.displayName ?? worktree.name;
		if (worktree.refName) {
			const subject = worktree.refName.fullNameBytes;
			return { branchName, relativeTo: { type: "referenceBytes", subject }, side: "below" };
		}
		return { branchName, relativeTo: { type: "commit", subject: worktree.head }, side: "above" };
	}

	/** A stack commits where the commit box points, into a new stack if needed. */
	async function stackPlan(): Promise<CommitPlan> {
		let finalStackId = stackId;
		let branchName: string | undefined =
			commitAction?.branchName ||
			draftBranchName ||
			topBranchName ||
			(await stackService.fetchNewBranchName(projectId));

		if (!finalStackId) {
			const stack = await createNewStack({ projectId, branch: { name: branchName, order: 0 } });
			finalStackId = stack.id ?? undefined;
			branchName = stack.heads[0]?.name;
			uiState.global.draftBranchName.set(undefined);
		}
		if (!finalStackId) throw new Error("No stack selected!");
		if (!branchName) throw new Error("No branch selected!");

		return {
			...toCommitCreatePlacement({
				stackBranchName: branchName,
				parentId: commitAction?.parentCommitId,
				insertBelow: commitAction?.insertBelow,
			}),
			branchName,
			stackId: finalStackId,
		};
	}

	/** The message after the commit-msg hook, or `undefined` when it failed. */
	async function hookedMessage(message: string): Promise<string | undefined> {
		const result = await runMessageHook({ projectId, message });
		if (result?.status === "failure") {
			showWarning("Commit message hook failed", result.error);
			return undefined;
		}
		if (result?.status === "message") return result.message;
		return message;
	}

	async function createCommit(rawMessage: string) {
		if (isCooking) {
			showToast({ message: "Commit is already in progress", style: "danger" });
			return;
		}

		isCooking = true;
		await tick();
		try {
			const changes = await uncommittedService.worktreeChanges(projectId, stackId);
			let plan: CommitPlan;
			if (worktree) plan = worktreePlan(worktree);
			else plan = await stackPlan();

			const runHooks = $runCommitHooks && !worktree;
			let message: string | undefined = rawMessage;
			if (runHooks) message = await hookedMessage(rawMessage);
			if (message === undefined) return;

			let properties = {};
			if (plan.stackId) {
				properties = await commitAnalytics.getCommitProperties({
					projectId,
					stackId: plan.stackId,
					selectedBranchName: plan.branchName,
					message,
					changes,
					parentId: commitAction?.parentCommitId,
					isRichTextMode: input?.isRichTextMode?.() || false,
				});
			}

			if (runHooks) await hooksService.runPreCommitHooks(projectId, changes);
			const { relativeTo, side } = plan;
			const response = await createCommitMutation(
				{ projectId, relativeTo, side, changes, worktree: worktree?.name, message },
				{ properties },
			);
			if (runHooks) await hooksService.runPostCommitHooks(projectId);

			const newCommitId = response.newCommit ?? undefined;
			if (newCommitId) {
				laneState.newCommitMessage.set({ title: "", description: "" });
				idSelection.clear(createWorktreeSelection({ stackId, worktree: worktree?.name }));
				projectState.exclusiveAction.set(undefined);
				uncommittedService.clearHunkSelection();
			}

			if (response.rejectedChanges.length > 0) {
				uiState.global.modal.set({
					type: "commit-failed",
					projectId,
					targetBranchName: plan.branchName,
					newCommitId,
					commitTitle: laneState.newCommitMessage.current?.title || "",
					pathsToRejectedChanges: rejectedChangesByPath(response),
				});
			}
		} finally {
			isCooking = false;
		}
	}

	async function handleCommitCreation(title: string, description: string) {
		laneState.newCommitMessage.set({ title, description });

		const message = description ? title + "\n\n" + description : title;
		if (!message) {
			showToast({ message: "Commit message is required", style: "danger" });
			return;
		}

		// HookFailedError extends SilentError, so any toast it would
		// otherwise produce is suppressed by the classifier; every other
		// failure bubbles to the global handler.
		await createCommit(message);
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
	}
</script>

<div data-testid={TestId.NewCommitView}>
	<CommitMessageEditor
		bind:this={input}
		{projectId}
		{stackId}
		worktree={worktree?.name}
		actionLabel="Create commit"
		floatingBoxHeader={worktree && `Commit to ${worktree.name}`}
		action={({ title, description }) => handleCommitCreation(title, description)}
		onChange={({ title, description }) => handleMessageUpdate(title, description)}
		onCancel={cancel}
		disabledAction={selectedLines.current.length === 0}
		loading={commitCreation.current.isLoading || newStackQuery.current.isLoading || isCooking}
		title={laneState.newCommitMessage.current.title}
		description={laneState.newCommitMessage.current.description}
	/>
</div>
