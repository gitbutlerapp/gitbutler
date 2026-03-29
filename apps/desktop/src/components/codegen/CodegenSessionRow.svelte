<script lang="ts">
	import Dropzone from "$components/shared/Dropzone.svelte";
	import DropzoneOverlay from "$components/shared/DropzoneOverlay.svelte";
	import ErrorBoundary from "$components/shared/ErrorBoundary.svelte";
	import ReduxResult from "$components/shared/ReduxResult.svelte";
	import { ATTACHMENT_SERVICE } from "$lib/codegen/attachmentService.svelte";
	import { CLAUDE_CODE_SERVICE } from "$lib/codegen/claude";
	import {
		CodegenCommitDropHandler,
		CodegenFileDropHandler,
		CodegenHunkDropHandler,
	} from "$lib/codegen/codegenDropzone";
	import {
		extractLastMessage,
		formatMessages,
		getTodos,
		hasPendingAskUserQuestion,
		userFeedbackStatus,
	} from "$lib/codegen/messages";
	import { combineResults } from "$lib/state/helpers";
	import { UI_STATE } from "$lib/state/uiState.svelte";
	import { truncate } from "$lib/utils/string";
	import { inject } from "@gitbutler/core/context";
	import { Badge, Icon, type IconName } from "@gitbutler/ui";
	import { focusable } from "@gitbutler/ui/focus/focusable";
	import { slide } from "svelte/transition";
	import type { ClaudeStatus, PromptAttachment } from "$lib/codegen/types";

	type Props = {
		projectId?: string;
		stackId?: string;
		branchName?: string;
		selected?: boolean;
		status?: ClaudeStatus;
		onclick?: () => void;
		draft?: boolean;
	};

	const {
		projectId,
		stackId,
		branchName,
		selected = false,
		status,
		onclick,
		draft = false,
	}: Props = $props();

	const uiState = draft ? undefined : inject(UI_STATE);
	const laneState = draft || !stackId ? undefined : uiState?.lane(stackId);

	const claudeService = draft ? undefined : inject(CLAUDE_CODE_SERVICE);
	const messages =
		draft || !projectId || !stackId ? undefined : claudeService?.messages({ projectId, stackId });
	const permissionRequests = $derived(
		draft || !projectId ? undefined : claudeService?.permissionRequests({ projectId }),
	);

	let active = $state(false);

	function getCurrentIconName(hasPendingApproval: boolean): IconName {
		if (hasPendingApproval) {
			return "danger";
		}

		if (status === "running" || status === "compacting") {
			return "spinner";
		}
		return "ai";
	}

	const attachmentService = draft ? undefined : inject(ATTACHMENT_SERVICE);

	function addAttachment(items: PromptAttachment[]) {
		if (!branchName || !attachmentService) return;
		return attachmentService.add(branchName, items);
	}

	const handlers = $derived(
		draft || !stackId || !branchName
			? []
			: [
					new CodegenCommitDropHandler(stackId, addAttachment),
					new CodegenFileDropHandler(stackId, branchName, addAttachment),
					new CodegenHunkDropHandler(stackId, addAttachment),
				],
	);

	function toggleSelection() {
		if (draft) return; // Don't allow selection in draft mode
		if (!branchName || !laneState) return;
		laneState.selection.set(
			selected ? undefined : { branchName, codegen: true, previewOpen: true },
		);
		onclick?.();
	}
</script>

<Dropzone {handlers}>
	{#snippet overlay({ hovered, activated })}
		<DropzoneOverlay {hovered} {activated} label="Reference" />
	{/snippet}
	<ErrorBoundary compact>
		{#if draft}
			<!-- Draft mode: simple placeholder -->
			<div class="codegen-row">
				<Icon name="ai" color="var(--fill-purple-bg)" />
				<h3 class="text-13 text-semibold truncate codegen-row__title">
					AI session will start here
				</h3>
			</div>
		{:else if projectId && messages && permissionRequests}
			<ReduxResult {projectId} result={combineResults(messages.result, permissionRequests.result)}>
				{#snippet children([messages, permissionReqs])}
					{@const lastMessage = extractLastMessage(messages)}
					{@const lastSummary = lastMessage ? truncate(lastMessage, 360, 8) : undefined}
					{@const todos = getTodos(messages)}
					{@const completedCount = todos.filter((t) => t.status === "completed").length}
					{@const totalCount = todos.length}
					{@const formattedMessages = formatMessages(
						messages,
						permissionReqs,
						status === "running",
					)}
					{@const feedbackStatus = userFeedbackStatus(formattedMessages)}
					{@const hasPendingQuestion = hasPendingAskUserQuestion(formattedMessages)}
					{@const hasPendingApproval = feedbackStatus.waitingForFeedback}
					{@const hasPendingAction = hasPendingApproval || hasPendingQuestion}

					<button
						type="button"
						class="codegen-row"
						class:selected
						class:active
						class:codegen-row--wiggle={hasPendingAction && !selected}
						onclick={toggleSelection}
						use:focusable={{
							onAction: toggleSelection,
							onActive: (value) => (active = value),
							focusable: true,
						}}
					>
						{#if selected}
							<div
								class="indicator"
								class:selected
								class:active
								in:slide={{ axis: "x", duration: 150 }}
							></div>
						{/if}

						<Icon name={getCurrentIconName(hasPendingAction)} color="var(--fill-purple-bg)" />
						<h3 class="text-13 text-semibold truncate codegen-row__title">{lastSummary}</h3>

						{#if hasPendingAction}
							<Badge style="pop" tooltip="Waiting for action">Action needed</Badge>
						{/if}

						{#if totalCount > 1}
							<span class="text-12 codegen-row__todos">Todos ({completedCount}/{totalCount})</span>

							{#if completedCount === totalCount}
								<Icon name="tick-circle" color="var(--fill-safe-bg)" />
							{/if}
						{/if}
					</button>
				{/snippet}
			</ReduxResult>
		{/if}
	</ErrorBoundary>
</Dropzone>

<style lang="postcss">
	.codegen-row {
		display: flex;
		position: relative;
		align-items: center;
		width: 100%;
		height: 44px;
		padding: 0 12px;
		padding-left: 14px;
		gap: 8px;
		border: 1px solid var(--border-2);
		border-radius: var(--radius-ml);
		background-color: var(--bg-purple);
		text-align: left;

		transition: background-color var(--transition-fast);

		&.active.selected,
		&[type="button"]:hover {
			background-color: var(--hover-purple-bg);
		}
	}

	.codegen-row--wiggle {
		animation: row-wiggle 5s ease-in-out infinite;
	}

	.codegen-row__title {
		flex: 1;
		color: var(--chip-purple-fg);
	}

	.codegen-row__todos {
		flex-shrink: 0;
		color: var(--chip-purple-fg);
		opacity: 0.7;
	}

	.indicator {
		position: absolute;
		top: 50%;
		left: 0;
		width: 4px;
		height: 45%;
		transform: translateY(-50%);
		border-radius: 0 var(--radius-ml) var(--radius-ml) 0;
		background-color: var(--fill-purple-bg);
		transition: transform var(--transition-fast);
	}
</style>
