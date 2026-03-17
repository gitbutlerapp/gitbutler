<!--
	Compound child that owns all codegen state and rendering for the stack details panel.
	Reads shared state from StackController via context.

	Usage:
	```svelte
	<StackCodegen {hasRulesToClear} {claudeConfig} onclose={() => controller.closePreview()} />
	```
-->
<script lang="ts">
	import CodegenMessages from "$components/codegen/CodegenMessages.svelte";
	import CodegenMcpConfigModal from "$components/codegen/CodegenMcpConfigModal.svelte";
	import ReduxResult from "$components/ReduxResult.svelte";
	import { ATTACHMENT_SERVICE } from "$lib/codegen/attachmentService.svelte";
	import { CLAUDE_CODE_SERVICE } from "$lib/codegen/claude";
	import { MessageSender } from "$lib/codegen/messageQueue.svelte";
	import { RULES_SERVICE } from "$lib/rules/rulesService.svelte";
	import { getStackContext } from "$lib/stack/stackController.svelte";
	import { inject } from "@gitbutler/core/context";
	import { reactive } from "@gitbutler/shared/reactiveUtils.svelte";
	import type { ClaudeConfig } from "$lib/codegen/types";

	type Props = {
		hasRulesToClear: boolean;
		claudeConfig: ClaudeConfig;
		branchName: string;
		onclose: () => void;
	};

	const { hasRulesToClear, claudeConfig, branchName, onclose }: Props = $props();

	const controller = getStackContext();

	// ── Injected services ────────────────────────────────────────────
	const claudeCodeService = inject(CLAUDE_CODE_SERVICE);
	const rulesService = inject(RULES_SERVICE);
	const attachmentService = inject(ATTACHMENT_SERVICE);

	// ── Codegen state ────────────────────────────────────────────────
	const isStackActiveQuery = $derived(
		claudeCodeService.isStackActive(controller.projectId, controller.stackId),
	);
	const isStackActive = $derived(isStackActiveQuery?.response || false);
	const events = $derived(
		claudeCodeService.messages({
			projectId: controller.projectId,
			stackId: controller.stackId,
		}),
	);
	const sessionIdQuery = $derived(
		rulesService.aiSessionId(controller.projectId, controller.stackId),
	);
	const permissionRequests = $derived(
		claudeCodeService.permissionRequests({ projectId: controller.projectId }),
	);
	const attachments = $derived(attachmentService.getByBranch(controller.branchName));

	const selectedThinkingLevel = $derived(controller.projectState.thinkingLevel.current);
	const selectedModel = $derived(controller.projectState.selectedModel.current);
	const selectedPermissionMode = $derived(controller.laneState.permissionMode.current);

	const messageSender = $derived(
		controller.stackId && controller.branchName
			? new MessageSender({
					projectId: reactive(() => controller.projectId),
					selectedBranch: reactive(() => ({
						stackId: controller.stackId!,
						head: controller.branchName!,
					})),
					thinkingLevel: reactive(() => selectedThinkingLevel),
					model: reactive(() => selectedModel),
					permissionMode: reactive(() => selectedPermissionMode),
				})
			: undefined,
	);
	const initialPrompt = $derived(messageSender?.prompt);

	// ── Actions ──────────────────────────────────────────────────────
	async function onAbort() {
		if (controller.stackId) {
			await claudeCodeService.cancelSession({
				projectId: controller.projectId,
				stackId: controller.stackId,
			});
		}
	}

	async function sendMessage(prompt: string) {
		await messageSender?.sendMessage(prompt, attachments);
		attachmentService.clearByBranch(controller.branchName);
	}

	async function handleAnswerQuestion(answers: Record<string, string>) {
		if (!controller.stackId) return;
		await claudeCodeService.answerAskUserQuestion({
			projectId: controller.projectId,
			stackId: controller.stackId,
			answers,
		});
	}

	let mcpConfigModal = $state<CodegenMcpConfigModal>();
	const mcpClaudeConfigQuery = $derived(
		claudeCodeService.claudeConfig({ projectId: controller.projectId }),
	);
</script>

<CodegenMessages
	projectId={controller.projectId}
	stackId={controller.stackId}
	laneId={controller.laneId}
	{branchName}
	{onclose}
	onMcpSettings={() => {
		mcpConfigModal?.open();
	}}
	{onAbort}
	{initialPrompt}
	events={events.response || []}
	permissionRequests={permissionRequests.response || []}
	onSubmit={sendMessage}
	onChange={(prompt) => messageSender?.setPrompt(prompt)}
	sessionId={sessionIdQuery.response}
	{isStackActive}
	{hasRulesToClear}
	projectRegistered={claudeConfig.projectRegistered}
	onRetryConfig={async () => {
		await claudeCodeService.fetchClaudeConfig(
			{ projectId: controller.projectId },
			{ forceRefetch: true },
		);
	}}
	onAnswerQuestion={handleAnswerQuestion}
/>

<!-- MCP CONFIG MODAL -->
<ReduxResult
	result={mcpClaudeConfigQuery.result}
	projectId={controller.projectId}
	stackId={controller.stackId}
	hideError
>
	{#snippet children(config, { stackId: resolvedStackId })}
		{@const resolvedLaneState = resolvedStackId
			? controller.uiState.lane(resolvedStackId)
			: undefined}
		<CodegenMcpConfigModal
			disabledServers={resolvedLaneState?.disabledMcpServers.current || []}
			bind:this={mcpConfigModal}
			claudeConfig={config}
			toggleServer={(server) => {
				const disabledServers = resolvedLaneState?.disabledMcpServers.current;
				if (disabledServers) {
					if (disabledServers.includes(server)) {
						resolvedLaneState?.disabledMcpServers.set(disabledServers.filter((s) => s !== server));
					} else {
						resolvedLaneState?.disabledMcpServers.set([...disabledServers, server]);
					}
				}
			}}
		/>
	{/snippet}
</ReduxResult>
