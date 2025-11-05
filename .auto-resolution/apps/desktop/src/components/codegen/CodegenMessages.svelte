<script lang="ts">
	import { goto } from '$app/navigation';
	import BranchHeaderIcon from '$components/BranchHeaderIcon.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';

	import ClaudeCodeSettingsModal from '$components/codegen/ClaudeCodeSettingsModal.svelte';
	import CodegenChatClaudeNotAvaliableBanner from '$components/codegen/CodegenChatClaudeNotAvaliableBanner.svelte';
	import CodegenChatLayout from '$components/codegen/CodegenChatLayout.svelte';
	import CodegenClaudeMessage from '$components/codegen/CodegenClaudeMessage.svelte';
	import CodegenInput from '$components/codegen/CodegenInput.svelte';
	import CodegenMcpConfigModal from '$components/codegen/CodegenMcpConfigModal.svelte';
	import CodegenPromptConfigModal from '$components/codegen/CodegenPromptConfigModal.svelte';
	import CodegenServiceMessageThinking from '$components/codegen/CodegenServiceMessageThinking.svelte';
	import CodegenServiceMessageUseTool from '$components/codegen/CodegenServiceMessageUseTool.svelte';
	import CodegenTodoAccordion from '$components/codegen/CodegenTodoAccordion.svelte';
	import laneNewSvg from '$lib/assets/empty-state/lane-new.svg?raw';
	import { ATTACHMENT_SERVICE } from '$lib/codegen/attachmentService.svelte';
	import { CLAUDE_CODE_SERVICE } from '$lib/codegen/claude';
	import { MessageSender } from '$lib/codegen/messageQueue.svelte';
	import {
		currentStatus,
		thinkingOrCompactingStartedAt,
		userFeedbackStatus,
		usageStats,
		formatMessages,
		getTodos
	} from '$lib/codegen/messages';

	import { SETTINGS_SERVICE } from '$lib/config/appSettingsV2';
	import { vscodePath } from '$lib/project/project';
	import { PROJECTS_SERVICE } from '$lib/project/projectsService';
	import { workspacePath } from '$lib/routes/routes.svelte';
	import { RULES_SERVICE } from '$lib/rules/rulesService.svelte';
	import { SETTINGS } from '$lib/settings/userSettings';
	import { pushStatusToColor, pushStatusToIcon } from '$lib/stacks/stack';
	import { STACK_SERVICE } from '$lib/stacks/stackService.svelte';
	import { combineResults } from '$lib/state/helpers';
	import { UI_STATE } from '$lib/state/uiState.svelte';
	import { formatCompactNumber } from '$lib/utils/number';
	import { getEditorUri, URL_SERVICE } from '$lib/utils/url';
	import { inject } from '@gitbutler/core/context';
	import { reactive } from '@gitbutler/shared/reactiveUtils.svelte';
	import {
		Badge,
		Button,
		chipToasts,
		ContextMenu,
		ContextMenuItem,
		ContextMenuSection,
		EmptyStatePlaceholder,
		KebabButton,
		Modal,
		Tooltip
	} from '@gitbutler/ui';

	import VirtualList from '@gitbutler/ui/components/VirtualList.svelte';
	import { getColorFromBranchType } from '@gitbutler/ui/utils/getColorFromBranchType';
	import type { ClaudeMessage, ThinkingLevel, ModelType, PermissionMode } from '$lib/codegen/types';

	type Props = {
		projectId: string;
		branchName: string;
		stackId: string;
		isWorkspace?: boolean;
		onclose?: () => void;
	};
	const { projectId, stackId, branchName, isWorkspace, onclose }: Props = $props();

	const stableBranchName = $derived(branchName);

	const attachmentService = inject(ATTACHMENT_SERVICE);
	const claudeCodeService = inject(CLAUDE_CODE_SERVICE);
	const stackService = inject(STACK_SERVICE);
	const projectsService = inject(PROJECTS_SERVICE);
	const rulesService = inject(RULES_SERVICE);
	const uiState = inject(UI_STATE);
	const urlService = inject(URL_SERVICE);
	const userSettings = inject(SETTINGS);
	const settingsService = inject(SETTINGS_SERVICE);
	const claudeSettings = $derived($settingsService?.claude);

	const stacks = $derived(stackService.stacks(projectId));
	const claudeAvailable = $derived(claudeCodeService.checkAvailable(undefined));
	const mcpConfigQuery = $derived(claudeCodeService.mcpConfig({ projectId }));
	const attachments = $derived(attachmentService.getByBranch(branchName));

	let settingsModal: ClaudeCodeSettingsModal | undefined;
	let clearContextModal = $state<Modal>();
	let modelContextMenu = $state<ContextMenu>();
	let modelTrigger = $state<HTMLButtonElement>();
	let thinkingModeContextMenu = $state<ContextMenu>();
	let thinkingModeTrigger = $state<HTMLButtonElement>();
	let permissionModeContextMenu = $state<ContextMenu>();
	let permissionModeTrigger = $state<HTMLButtonElement>();
	let templateContextMenu = $state<ContextMenu>();
	let templateTrigger = $state<HTMLButtonElement>();

	let mcpConfigModal = $state<CodegenMcpConfigModal>();
	let promptConfigModal = $state<CodegenPromptConfigModal>();

	const modelOptions: { label: string; value: ModelType }[] = [
		{ label: 'Haiku', value: 'haiku' },
		{ label: 'Sonnet', value: 'sonnet' },
		{ label: 'Sonnet 1m', value: 'sonnet[1m]' },
		{ label: 'Opus', value: 'opus' },
		{ label: 'Opus Planning', value: 'opusplan' }
	];

	const thinkingLevels: { label: string; shortLabel: string; value: ThinkingLevel }[] = [
		{ label: 'Normal', shortLabel: 'Normal', value: 'normal' },
		{ label: 'Think', shortLabel: 'Think', value: 'think' },
		{ label: 'Mega think', shortLabel: 'Mega', value: 'megaThink' },
		{ label: 'Ultra think', shortLabel: 'Ultra', value: 'ultraThink' }
	];

	const permissionModeOptions: { label: string; value: PermissionMode }[] = [
		{ label: 'Edit with permission', value: 'default' },
		{ label: 'Planning', value: 'plan' },
		{ label: 'Accept edits', value: 'acceptEdits' }
	];

	const promptTemplates = $derived(claudeCodeService.promptTemplates(projectId));
	const promptDirs = $derived(claudeCodeService.promptDirs(projectId));

	async function openPromptConfigDir(path: string) {
		await claudeCodeService.createPromptDir({ projectId, path });

		const editorUri = getEditorUri({
			schemeId: $userSettings.defaultCodeEditor.schemeIdentifer,
			path: [path]
		});

		urlService.openExternalUrl(editorUri);
	}

	const projectState = uiState.project(projectId);
	const selectedThinkingLevel = $derived(projectState.thinkingLevel.current);
	const selectedModel = $derived(projectState.selectedModel.current);
	const selectedPermissionMode = $derived(uiState.lane(stackId).permissionMode.current);

	$effect(() => {
		if (stacks.response) {
			// Make sure the current selection is valid
			const branchFound = stacks.response.some(
				(s) => s.id === stackId && s.heads.some((h) => h.name === stableBranchName)
			);
			if (!branchFound) {
				selectFirstBranch();
			}
		}
	});

	function selectFirstBranch() {
		if (!stacks.response) return;

		const firstStack = stacks.response[0];
		const firstHead = firstStack?.heads[0];
		if (firstHead && firstStack.id) {
			projectState.selectedClaudeSession.set({
				stackId: firstStack.id,
				head: firstHead.name
			});
		} else {
			projectState.selectedClaudeSession.set(undefined);
		}
	}

	async function onApproval(id: string) {
		await claudeCodeService.updatePermissionRequest({ projectId, requestId: id, approval: true });
	}
	async function onRejection(id: string) {
		await claudeCodeService.updatePermissionRequest({ projectId, requestId: id, approval: false });
	}
	async function onAbort() {
		await claudeCodeService.cancelSession({ projectId, stackId });
	}

	function selectModel(model: ModelType) {
		projectState.selectedModel.set(model);
		modelContextMenu?.close();
	}

	function selectThinkingLevel(level: ThinkingLevel) {
		projectState.thinkingLevel.set(level);
		thinkingModeContextMenu?.close();
	}

	function selectPermissionMode(mode: PermissionMode) {
		uiState.lane(stackId).permissionMode.set(mode);
		permissionModeContextMenu?.close();
	}

	function getPermissionModeIcon(
		mode: PermissionMode
	): 'edit-with-permissions' | 'checklist' | 'allow-all' {
		switch (mode) {
			case 'default':
				return 'edit-with-permissions';
			case 'plan':
				return 'checklist';
			case 'acceptEdits':
				return 'allow-all';
			default:
				return 'edit-with-permissions';
		}
	}

	function thinkingLevelToUiLabel(level: ThinkingLevel, short: boolean = false): string {
		const thinkingLevel = thinkingLevels.find((t) => t.value === level);
		if (!thinkingLevel) return 'Normal';
		return short ? thinkingLevel.shortLabel : thinkingLevel.label;
	}

	const messageSender = new MessageSender({
		projectId: reactive(() => projectId),
		selectedBranch: reactive(() => ({ stackId, head: stableBranchName })),
		thinkingLevel: reactive(() => selectedThinkingLevel),
		model: reactive(() => selectedModel),
		permissionMode: reactive(() => selectedPermissionMode)
	});

	const initialPrompt = $state.snapshot(messageSender.prompt);

	async function sendMessage(prompt: string) {
		await messageSender.sendMessage(prompt, attachments);
		attachmentService.clearByBranch(branchName);
	}

	function insertTemplate(template: string) {
		const currentPrompt = messageSender.prompt;
		messageSender.setPrompt(currentPrompt + (currentPrompt ? '\n\n' : '') + template);
		templateContextMenu?.close();
	}

	function showInWorkspace() {
		goto(`${workspacePath(projectId)}?stackId=${stackId}`);
	}

	async function openInEditor() {
		const project = await projectsService.fetchProject(projectId);
		if (!project) {
			chipToasts.error('Project not found');
			return;
		}
		urlService.openExternalUrl(
			getEditorUri({
				schemeId: $userSettings.defaultCodeEditor.schemeIdentifer,
				path: [vscodePath(project.path)],
				searchParams: { windowId: '_blank' }
			})
		);
	}

	function getCurrentSessionId(events: ClaudeMessage[]): string | undefined {
		// Get the most recent session ID from the messages
		if (events.length === 0) return undefined;
		const lastEvent = events[events.length - 1];
		return lastEvent?.sessionId;
	}

	function clearContextAndRules() {
		clearContextModal?.show();
	}

	async function compactContext() {
		await claudeCodeService.compactHistory({
			projectId,
			stackId
		});
	}

	async function performClearContextAndRules() {
		const events = await claudeCodeService.fetchMessages({
			projectId,
			stackId
		});
		const sessionId = getCurrentSessionId(events);
		if (!sessionId) return;

		const rules = await rulesService.fetchListWorkspaceRules(projectId);

		const toDelete = rules.filter((rule) =>
			rule.filters.some(
				(filter) => filter.type === 'claudeCodeSessionId' && filter.subject === sessionId
			)
		);

		for (const rule of toDelete) {
			await rulesService.deleteWorkspaceRuleMutate({
				projectId,
				id: rule.id
			});
		}
	}

	const isStackActiveQuery = $derived(claudeCodeService.isStackActive(projectId, stackId));
	const isStackActive = $derived(isStackActiveQuery?.response || false);

	// Check if there are rules to delete for the current session
	const rules = $derived(rulesService.workspaceRules(projectId));
	const hasRulesToClear = $derived(() => {
		if (!events.response || !rules.response) return false;

		const sessionId = getCurrentSessionId(events.response);
		if (!sessionId) return false;

		return rules.response.some((rule) =>
			rule.filters.some(
				(filter) => filter.type === 'claudeCodeSessionId' && filter.subject === sessionId
			)
		);
	});

	const events = $derived(claudeCodeService.messages({ projectId, stackId }));
	const permissionRequests = $derived(claudeCodeService.permissionRequests({ projectId }));
	const selectedBranchDetails = $derived(
		stackService.branchDetails(projectId, stackId, stableBranchName)
	);
</script>

<ReduxResult
	result={combineResults(
		events?.result,
		permissionRequests.result,
		selectedBranchDetails.result,
		mcpConfigQuery.result
	)}
	{projectId}
>
	{#snippet children(
		[events, permissionRequests, branchDetailsData, mcpConfig],
		{ projectId: _projectId }
	)}
		{@const formattedMessages = formatMessages(events, permissionRequests, isStackActive)}
		{@const iconName = pushStatusToIcon(branchDetailsData.pushStatus)}
		{@const lineColor = getColorFromBranchType(pushStatusToColor(branchDetailsData.pushStatus))}
		{@const enabledMcpServers = mcpConfig
			? Object.keys(mcpConfig.mcpServers).length -
				uiState.lane(stackId).disabledMcpServers.current.length
			: 0}
		<CodegenChatLayout branchName={stableBranchName} {isWorkspace} {onclose}>
			{#snippet branchIcon()}
				<BranchHeaderIcon {iconName} color={lineColor} large />
			{/snippet}
			{#snippet inWorkspaceInlineContextActions()}
				{@const stats = usageStats(events)}
				{@const contextUsage = Math.round(stats.contextUtilization * 100)}

				<div class="flex gap-10 items-center">
					{#if stats.tokens > 0}
						<Tooltip text="Tokens: {stats.tokens.toLocaleString()} / ${stats.cost.toFixed(2)} ">
							<span class="text-12 clr-text-2">
								{formatCompactNumber(stats.tokens)}
							</span>
						</Tooltip>
					{/if}

					<Tooltip text="{contextUsage}% context used">
						<div class="context-utilization-scale" style="--context-utilization: {contextUsage}">
							<svg viewBox="0 0 17 17">
								<circle class="bg-circle" cx="8.5" cy="8.5" r="6.5" />
								<circle class="progress-circle" cx="8.5" cy="8.5" r="6.5" />
							</svg>
						</div>
					</Tooltip>

					<KebabButton>
						{#snippet contextMenu({ close })}
							{@const isDisabled =
								!hasRulesToClear() ||
								!events ||
								events.length === 0 ||
								['running', 'compacting'].includes(currentStatus(events, isStackActive))}

							<ContextMenuSection>
								<ContextMenuItem
									label="MCP settings"
									icon="mcp"
									onclick={() => {
										mcpConfigModal?.open();
										close();
									}}
								/>
								<ContextMenuItem
									label="Agent settings"
									icon="mixer"
									onclick={() => {
										settingsModal?.show();
										close();
									}}
								/>
							</ContextMenuSection>
							<ContextMenuSection>
								<ContextMenuItem
									label="Clear context and rules"
									icon="clear"
									disabled={isDisabled}
									onclick={() => {
										clearContextAndRules();
										close();
									}}
								/>
								<ContextMenuItem
									label="Compact context"
									icon="compact"
									disabled={isDisabled}
									onclick={() => {
										compactContext();
										close();
									}}
								/>
							</ContextMenuSection>
						{/snippet}
					</KebabButton>
				</div>
			{/snippet}
			{#snippet pageWorkspaceActions()}
				<Button
					icon="workbench-small"
					kind="outline"
					size="tag"
					reversedDirection
					onclick={showInWorkspace}>Show in workspace</Button
				>
				<Button
					icon="open-editor-small"
					kind="outline"
					size="tag"
					tooltip="Open in {$userSettings.defaultCodeEditor.displayName}"
					onclick={openInEditor}
					reversedDirection
				/>
			{/snippet}

			{#snippet pageContextActions()}
				{@const stats = usageStats(events)}

				<Button kind="outline" icon="mcp" reversedDirection onclick={() => mcpConfigModal?.open()}
					>MCP

					{#snippet badge()}
						<Badge kind="soft">{enabledMcpServers}</Badge>
					{/snippet}
				</Button>

				<div class="flex gap-4 overflow-hidden">
					<div
						class="text-11 context-utilization-badge-2"
						style="--context-utilization: {stats.contextUtilization}"
					>
						<span class="truncate">
							{Math.round(stats.contextUtilization * 100)}% context used
						</span>
					</div>

					<div class="flex">
						<Button
							icon="clear"
							kind="outline"
							tooltip="Clear context and associated rules"
							disabled={!hasRulesToClear() ||
								!events ||
								events.length === 0 ||
								['running', 'compacting'].includes(currentStatus(events, isStackActive))}
							customStyle="border-top-right-radius: 0; border-bottom-right-radius: 0;"
							onclick={() => clearContextAndRules()}
						/>
						<Button
							icon="compact"
							kind="outline"
							tooltip="Compact context"
							customStyle="border-top-left-radius: 0; border-bottom-left-radius: 0; border-left: none;"
							disabled={!hasRulesToClear() ||
								!events ||
								events.length === 0 ||
								['running', 'compacting'].includes(currentStatus(events, isStackActive))}
							onclick={() => compactContext()}
						/>
					</div>
				</div>
			{/snippet}

			{#snippet messages()}
				{@const todos = getTodos(events)}
				{#if todos.length > 0}
					<CodegenTodoAccordion {todos} />
				{/if}

				{#if !isStackActive && formattedMessages.length === 0}
					<div class="chat-view__placeholder">
						<EmptyStatePlaceholder
							image={laneNewSvg}
							width={320}
							topBottomPadding={0}
							bottomMargin={0}
						>
							{#snippet title()}
								Let's build something amazing
							{/snippet}
							{#snippet caption()}
								Your branch is ready for AI-powered development. Describe what you'd like to build,
								and I'll generate the code to get you started.
							{/snippet}
						</EmptyStatePlaceholder>
					</div>
				{:else}
					<VirtualList
						grow
						tail
						stickToBottom
						items={formattedMessages}
						batchSize={1}
						visibility={$userSettings.scrollbarVisibilityState}
						padding={{ left: 20, right: 20, top: 12, bottom: 12 }}
						defaultHeight={150}
					>
						{#snippet chunkTemplate(messages)}
							{#each messages as message}
								<CodegenClaudeMessage {message} {onApproval} {onRejection} />
							{/each}
						{/snippet}
						{@const thinkingStatus = currentStatus(events, isStackActive)}
						{@const startAt = thinkingOrCompactingStartedAt(events)}
						{#if ['running', 'compacting'].includes(thinkingStatus) && startAt}
							{@const status = userFeedbackStatus(formattedMessages)}
							{#if status.waitingForFeedback}
								<CodegenServiceMessageUseTool toolCall={status.toolCall} />
							{:else}
								<CodegenServiceMessageThinking
									{startAt}
									msSpentWaiting={status.msSpentWaiting}
									overrideWord={thinkingStatus === 'compacting' ? 'compacting' : undefined}
								/>
							{/if}
						{/if}
					</VirtualList>
				{/if}
			{/snippet}

			{#snippet input()}
				{#if claudeAvailable.response?.status === 'not_available'}
					<CodegenChatClaudeNotAvaliableBanner onSettingsBtnClick={() => settingsModal?.show()} />
				{:else}
					{@const status = currentStatus(events, isStackActive)}
					<CodegenInput
						{projectId}
						{stackId}
						branchName={stableBranchName}
						value={initialPrompt}
						loading={['running', 'compacting'].includes(status)}
						compacting={status === 'compacting'}
						onChange={(prompt) => messageSender.setPrompt(prompt)}
						onsubmit={sendMessage}
						{onAbort}
					>
						{#snippet actionsOnLeft()}
							{@const permissionModeLabel = permissionModeOptions.find(
								(a) => a.value === selectedPermissionMode
							)?.label}

							<div class="flex m-right-4 gap-2">
								<Button
									bind:el={templateTrigger}
									kind="ghost"
									icon="script"
									tooltip="Insert template"
									onclick={(e) => templateContextMenu?.toggle(e)}
								/>
								<Button
									bind:el={thinkingModeTrigger}
									kind="ghost"
									icon="thinking"
									reversedDirection
									onclick={() => thinkingModeContextMenu?.toggle()}
									tooltip="Thinking mode"
									children={selectedThinkingLevel === 'normal' ? undefined : thinkingBtnText}
								/>
								<Button
									bind:el={permissionModeTrigger}
									kind="ghost"
									icon={getPermissionModeIcon(selectedPermissionMode)}
									shrinkable
									onclick={() => permissionModeContextMenu?.toggle()}
									tooltip={$settingsService?.claude.dangerouslyAllowAllPermissions
										? 'Permission modes disable when all permissions are allowed'
										: permissionModeLabel}
									disabled={$settingsService?.claude.dangerouslyAllowAllPermissions}
								/>
							</div>
						{/snippet}

						{#snippet actionsOnRight()}
							{#if !claudeSettings?.useConfiguredModel}
								<Button
									bind:el={modelTrigger}
									kind="ghost"
									icon="chevron-down"
									shrinkable
									onclick={() => modelContextMenu?.toggle()}
								>
									{modelOptions.find((a) => a.value === selectedModel)?.label}
								</Button>
							{/if}
						{/snippet}
					</CodegenInput>
				{/if}
			{/snippet}
		</CodegenChatLayout>
	{/snippet}
</ReduxResult>

<ReduxResult result={mcpConfigQuery.result} {projectId} {stackId}>
	{#snippet children(mcpConfig, { stackId })}
		{@const laneState = uiState.lane(stackId)}
		<CodegenMcpConfigModal
			bind:this={mcpConfigModal}
			{mcpConfig}
			disabledServers={laneState.disabledMcpServers.current}
			toggleServer={(server) => {
				const disabledServers = laneState.disabledMcpServers.current;
				if (disabledServers.includes(server)) {
					laneState.disabledMcpServers.set(disabledServers.filter((s) => s !== server));
				} else {
					laneState.disabledMcpServers.set([...disabledServers, server]);
				}
			}}
		/>
	{/snippet}
</ReduxResult>

{#snippet thinkingBtnText()}
	{thinkingLevelToUiLabel(selectedThinkingLevel, true)}
{/snippet}

<Modal
	bind:this={clearContextModal}
	width="small"
	type="warning"
	title="Clear context"
	onSubmit={async (close) => {
		await performClearContextAndRules();
		close();
	}}
>
	Are you sure you want to clear the context and delete all rules associated with this Claude
	session? This action cannot be undone.

	{#snippet controls(close)}
		<Button kind="outline" onclick={close}>Cancel</Button>
		<Button style="error" type="submit">Clear context</Button>
	{/snippet}
</Modal>

<ContextMenu bind:this={modelContextMenu} leftClickTrigger={modelTrigger} side="top" align="end">
	<ContextMenuSection>
		{#each modelOptions as option}
			<ContextMenuItem
				label={option.label}
				selected={selectedModel === option.value}
				onclick={() => selectModel(option.value)}
			/>
		{/each}
	</ContextMenuSection>
</ContextMenu>

<ContextMenu
	bind:this={thinkingModeContextMenu}
	leftClickTrigger={thinkingModeTrigger}
	align="start"
	side="top"
>
	<ContextMenuSection>
		{#each thinkingLevels as level}
			<ContextMenuItem
				label={level.label}
				selected={selectedThinkingLevel === level.value}
				onclick={() => selectThinkingLevel(level.value)}
			/>
		{/each}
	</ContextMenuSection>
</ContextMenu>

<ContextMenu
	bind:this={permissionModeContextMenu}
	leftClickTrigger={permissionModeTrigger}
	align="start"
	side="top"
>
	<ContextMenuSection>
		{#each permissionModeOptions as option}
			<ContextMenuItem
				label={option.label}
				selected={selectedPermissionMode === option.value}
				onclick={() => selectPermissionMode(option.value)}
			/>
		{/each}
	</ContextMenuSection>
</ContextMenu>

<ContextMenu
	bind:this={templateContextMenu}
	leftClickTrigger={templateTrigger}
	side="top"
	align="start"
>
	<ContextMenuSection>
		<ReduxResult result={promptTemplates.result} {projectId}>
			{#snippet children(promptTemplates, { projectId: _projectId })}
				{#each promptTemplates as template}
					<ContextMenuItem
						label={template.label}
						onclick={() => insertTemplate(template.template)}
					/>
				{/each}
			{/snippet}
		</ReduxResult>
	</ContextMenuSection>
	<ContextMenuSection>
		<ContextMenuItem
			label="Edit templates"
			icon="open-editor"
			onclick={() => promptConfigModal?.show()}
		/>
	</ContextMenuSection>
</ContextMenu>

{#if promptDirs.response}
	<CodegenPromptConfigModal
		bind:this={promptConfigModal}
		promptDirs={promptDirs.response}
		{openPromptConfigDir}
	/>
{/if}

<ClaudeCodeSettingsModal bind:this={settingsModal} onClose={() => {}} />

<style lang="postcss">
	.chat-view__placeholder {
		display: flex;
		flex: 1;
		align-items: center;
		justify-content: center;
		padding: 0 32px;
	}

	.context-utilization-scale {
		position: relative;
		width: 17px;
		height: 17px;
		transform: rotate(-90deg);

		& svg {
			width: 100%;
			height: 100%;
		}

		& circle {
			fill: none;
			stroke-width: 2;
			stroke-linecap: round;
		}

		& .bg-circle {
			stroke: color-mix(in srgb, var(--clr-text-2), transparent 85%);
		}

		& .progress-circle {
			stroke: var(--clr-text-2);
			stroke-dasharray: calc(3.14159 * 13);
			stroke-dashoffset: calc(3.14159 * 13 * (1 - var(--context-utilization) / 100));
			transition: stroke-dashoffset 0.3s ease;
		}
	}

	.context-utilization-badge-2 {
		display: flex;
		position: relative;
		align-items: center;
		justify-content: center;
		height: var(--size-button);
		padding: 0 8px;
		overflow: hidden;
		border-radius: var(--radius-m);
		background-color: var(--clr-theme-ntrl-soft);
		color: var(--clr-text-2);

		&::after {
			position: absolute;
			bottom: 0;
			left: 0;
			width: calc(var(--context-utilization) * 100%);
			height: 3px;
			background: var(--clr-text-3);
			content: '';
		}
	}
</style>
