<script lang="ts">
	import ConfigurableScrollableContainer from '$components/ConfigurableScrollableContainer.svelte';
	import PreviewHeader from '$components/PreviewHeader.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import AddedDirectories from '$components/codegen/AddedDirectories.svelte';
	import ClaudeCheck from '$components/codegen/ClaudeCheck.svelte';
	import CodegenChatClaudeNotAvaliableBanner from '$components/codegen/CodegenChatClaudeNotAvaliableBanner.svelte';
	import CodegenClaudeMessage from '$components/codegen/CodegenClaudeMessage.svelte';
	import CodegenInput from '$components/codegen/CodegenInput.svelte';
	import CodegenPromptConfigModal from '$components/codegen/CodegenPromptConfigModal.svelte';
	import CodegenServiceMessageThinking from '$components/codegen/CodegenServiceMessageThinking.svelte';
	import CodegenServiceMessageUseTool from '$components/codegen/CodegenServiceMessageUseTool.svelte';
	import CodegenTodoAccordion from '$components/codegen/CodegenTodoAccordion.svelte';
	import noClaudeCodeSvg from '$lib/assets/empty-state/claude-disconected.svg?raw';
	import laneNewSvg from '$lib/assets/empty-state/lane-new.svg?raw';
	import { CLAUDE_CODE_SERVICE } from '$lib/codegen/claude';
	import {
		currentStatus,
		thinkingOrCompactingStartedAt,
		userFeedbackStatus,
		usageStats,
		formatMessages,
		getTodos,
		type Message
	} from '$lib/codegen/messages';
	import { parseTemplates } from '$lib/codegen/templateParser';

	import { SETTINGS_SERVICE } from '$lib/config/appSettingsV2';
	import { RULES_SERVICE } from '$lib/rules/rulesService.svelte';
	import { SETTINGS } from '$lib/settings/userSettings';
	import { UI_STATE } from '$lib/state/uiState.svelte';
	import { formatCompactNumber } from '$lib/utils/number';
	import { getEditorUri, URL_SERVICE } from '$lib/utils/url';
	import { inject } from '@gitbutler/core/context';
	import {
		Button,
		ContextMenu,
		ContextMenuItem,
		ContextMenuSection,
		EmptyStatePlaceholder,
		KebabButton,
		Modal,
		Tooltip,
		Link,
		SkeletonBone
	} from '@gitbutler/ui';

	import VirtualList from '@gitbutler/ui/components/VirtualList.svelte';
	import { focusable } from '@gitbutler/ui/focus/focusable';
	import type {
		ClaudeMessage,
		ThinkingLevel,
		ModelType,
		PermissionMode,
		PermissionDecision,
		ClaudePermissionRequest
	} from '$lib/codegen/types';

	type Props = {
		projectId: string;
		branchName: string;
		stackId?: string;
		laneId: string;
		initialPrompt?: string;
		isStackActive?: boolean;
		events: ClaudeMessage[];
		permissionRequests: ClaudePermissionRequest[];
		sessionId?: string;
		hasRulesToClear?: boolean;
		onMcpSettings?: () => void;
		onclose?: () => void;
		onChange: (value: string) => void;
		onAbort?: () => Promise<void>;
		onSubmit?: (prompt: string) => Promise<void>;
	};
	const {
		projectId,
		stackId,
		laneId,
		branchName,
		initialPrompt,
		isStackActive = false,
		events,
		permissionRequests,
		sessionId,
		hasRulesToClear,
		onclose,
		onChange,
		onAbort,
		onSubmit,
		onMcpSettings
	}: Props = $props();

	const stableBranchName = $derived(branchName);

	const claudeCodeService = inject(CLAUDE_CODE_SERVICE);
	const rulesService = inject(RULES_SERVICE);
	const uiState = inject(UI_STATE);
	const urlService = inject(URL_SERVICE);
	const userSettings = inject(SETTINGS);
	const settingsService = inject(SETTINGS_SERVICE);
	const claudeSettings = $derived($settingsService?.claude);

	const claudeAvailable = $derived(claudeCodeService.checkAvailable(undefined));

	let clearContextModal = $state<Modal>();
	let modelContextMenu = $state<ContextMenu>();
	let modelTrigger = $state<HTMLButtonElement>();
	let thinkingModeContextMenu = $state<ContextMenu>();
	let thinkingModeTrigger = $state<HTMLButtonElement>();
	let permissionModeContextMenu = $state<ContextMenu>();
	let permissionModeTrigger = $state<HTMLButtonElement>();
	let templateContextMenu = $state<ContextMenu>();
	let templateTrigger = $state<HTMLButtonElement>();

	let promptConfigModal = $state<CodegenPromptConfigModal>();
	let virtualList = $state<VirtualList<Message>>();
	let inputRef = $state<CodegenInput>();

	// Track expanded state for tool calls by message createdAt timestamp
	const toolCallExpandedState = {
		groups: new Map<string, boolean>(),
		individual: new Map<string, boolean>()
	};

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

	// Parse templates once and cache the results
	const parsedTemplates = $derived(
		promptTemplates.response ? parseTemplates(promptTemplates.response) : []
	);

	async function openPromptConfigDir(path: string) {
		await claudeCodeService.createPromptDir({ projectId, path });

		const editorUri = getEditorUri({
			schemeId: $userSettings.defaultCodeEditor.schemeIdentifer,
			path: [path],
			searchParams: { windowId: '_blank' }
		});

		urlService.openExternalUrl(editorUri);
	}

	const projectState = uiState.project(projectId);
	const selectedThinkingLevel = $derived(projectState.thinkingLevel.current);
	const selectedModel = $derived(projectState.selectedModel.current);
	const selectedPermissionMode = $derived(uiState.lane(laneId).permissionMode.current);

	async function onPermissionDecision(
		id: string,
		decision: PermissionDecision,
		useWildcard: boolean
	) {
		await claudeCodeService.updatePermissionRequest({
			projectId,
			requestId: id,
			decision,
			useWildcard
		});
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
		uiState.lane(laneId).permissionMode.set(mode);
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

	async function insertTemplate(templateContent: string) {
		const currentPrompt = await inputRef?.getText();
		const newPrompt = currentPrompt + (currentPrompt ? '\n\n' : '') + templateContent;
		onChange?.(newPrompt);
		inputRef?.setText(newPrompt);
		templateContextMenu?.close();
	}

	// function getCurrentSessionId(events: ClaudeMessage[]): string | undefined {
	// 	// Get the most recent session ID from the messages
	// 	if (events.length === 0) return undefined;
	// 	const lastEvent = events[events.length - 1];
	// 	return lastEvent?.sessionId;
	// }

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

	const formattedMessages = $derived(formatMessages(events, permissionRequests, isStackActive));
</script>

<div class="chat" use:focusable={{ vertical: true }}>
	<ReduxResult result={claudeAvailable.result}>
		{#snippet loading()}
			<PreviewHeader {onclose}>
				{#snippet content()}
					<h3 class="text-14 text-semibold truncate">Chat for {branchName}</h3>
				{/snippet}
				{#snippet actions()}
					<div class="flex gap-4 items-center">
						<SkeletonBone width="2.5rem" height="1.2rem" />
						<SkeletonBone width="1.5rem" height="1.2rem" />
					</div>
				{/snippet}
			</PreviewHeader>

			<div class="chat-skeleton">
				<div class="chat-skeleton__user">
					<SkeletonBone
						width="80%"
						height="3rem"
						color="var(--clr-bg-3)"
						opacity={0.4}
						radius="var(--radius-ml) var(--radius-ml) 0 var(--radius-ml)"
					/>
				</div>
				<div class="chat-skeleton__assistant">
					<SkeletonBone width="100%" height="1rem" />
					<SkeletonBone width="90%" height="1rem" />
					<SkeletonBone width="95%" height="1rem" />
				</div>
				<div class="chat-skeleton__user">
					<SkeletonBone
						width="50%"
						height="3rem"
						color="var(--clr-bg-3)"
						opacity={0.4}
						radius="var(--radius-ml) var(--radius-ml) 0 var(--radius-ml)"
					/>
				</div>
				<div class="chat-skeleton__assistant">
					<SkeletonBone width="90%" height="1rem" />
					<SkeletonBone width="70%" height="1rem" />
				</div>
			</div>

			<div class="dialog-wrapper">
				<div class="input-skeleton">
					<div class="input-skeleton__text"><SkeletonBone width="60%" height="1rem" /></div>
					<div class="input-skeleton__actions">
						<SkeletonBone width="4rem" height="var(--size-button)" radius="var(--radius-btn)" />
						<div class="flex gap-8">
							<SkeletonBone width="4rem" height="var(--size-button)" radius="var(--radius-btn)" />
							<SkeletonBone
								width="calc(var(--size-button) + 0.188rem)"
								height="var(--size-button)"
								color="var(--clr-theme-pop-element)"
								radius="var(--radius-btn)"
							/>
						</div>
					</div>
				</div>
			</div>
		{/snippet}
		{#snippet children(claudeAvailable)}
			{@const todos = getTodos(events)}

			<!-- TODO: remove this header when we move to the workspace layout -->
			<PreviewHeader {onclose}>
				{#snippet content()}
					<h3 class="text-14 text-semibold truncate">Chat for {branchName}</h3>
				{/snippet}

				{#snippet actions()}
					{@const stats = usageStats(events)}
					{@const contextUsage = Math.round(stats.contextUtilization * 100)}

					<div class="flex gap-10 items-center">
						{#if stats.tokens > 0}
							<Tooltip text="Tokens: {stats.tokens.toLocaleString()} / ${stats.cost.toFixed(2)}">
								<span class="text-12 clr-text-2">
									{formatCompactNumber(stats.tokens)}
								</span>
							</Tooltip>

							<Tooltip text="{contextUsage}% context used">
								<div
									class="context-utilization-scale"
									style="--context-utilization: {contextUsage}"
								>
									<svg viewBox="0 0 17 17">
										<circle class="bg-circle" cx="8.5" cy="8.5" r="6.5" />
										<circle class="progress-circle" cx="8.5" cy="8.5" r="6.5" />
									</svg>
								</div>
							</Tooltip>
						{/if}

						<KebabButton>
							{#snippet contextMenu({ close })}
								{@const isDisabled =
									!hasRulesToClear ||
									!events ||
									events.length === 0 ||
									['running', 'compacting'].includes(currentStatus(events, isStackActive))}

								{#if onMcpSettings}
									<ContextMenuSection>
										<ContextMenuItem
											label="MCP settings"
											icon="mcp"
											onclick={() => {
												onMcpSettings?.();
												close();
											}}
										/>
									</ContextMenuSection>
								{/if}
								<ContextMenuSection>
									<ContextMenuItem
										label="Clear context"
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
			</PreviewHeader>

			<div class="chat-container">
				{#if claudeAvailable.status !== 'available' && formattedMessages.length === 0}
					<ConfigurableScrollableContainer childrenWrapDisplay="contents">
						<div class="no-agent-placeholder">
							<div class="no-agent-placeholder__content">
								{@html noClaudeCodeSvg}
								<h2 class="text-serif-42">Connect Claude Code</h2>
								<p class="text-13 text-body clr-text-2">
									If you haven't installed Claude Code, check our <Link
										class="clr-text-1"
										href="https://docs.gitbutler.com/features/agents-tab#installing-claude-code"
										>installation guide</Link
									>.
									<br />
									Click the button below to check if Claude Code is now available.
								</p>

								<ClaudeCheck />
							</div>

							<p class="text-12 text-body clr-text-2">
								Having trouble connecting?
								<br />
								Check the <Link href="https://docs.claude.com/en/docs/claude-code/troubleshooting"
									>troubleshooting guide</Link
								> for common issues and solutions.
							</p>
						</div>
					</ConfigurableScrollableContainer>
				{:else if !isStackActive && formattedMessages.length === 0}
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
								Your canvas is clear
								<br />
								Let the code take shape
							{/snippet}
						</EmptyStatePlaceholder>
					</div>
				{:else}
					<VirtualList
						bind:this={virtualList}
						grow
						stickToBottom
						items={formattedMessages}
						batchSize={1}
						visibility={$userSettings.scrollbarVisibilityState}
						padding={{ left: 20, right: 20, top: 12, bottom: 12 }}
						defaultHeight={65}
					>
						{#snippet chunkTemplate(messages)}
							{#each messages as message}
								<CodegenClaudeMessage
									{projectId}
									{message}
									{onPermissionDecision}
									{toolCallExpandedState}
								/>
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
				{#if todos.length > 0}
					<CodegenTodoAccordion {todos} />
				{/if}
			</div>
			{#if claudeAvailable.status !== 'available'}
				{#if formattedMessages.length > 0}
					<CodegenChatClaudeNotAvaliableBanner
						onSettingsBtnClick={() => {
							uiState.global.modal.set({
								type: 'project-settings',
								projectId,
								selectedId: 'agent'
							});
						}}
					/>
				{/if}
			{:else}
				{@const status = currentStatus(events, isStackActive)}
				{@const laneState = uiState.lane(laneId)}
				{@const addedDirs = laneState.addedDirs.current}

				<div class="dialog-wrapper">
					<AddedDirectories
						{addedDirs}
						onRemoveDir={(dir) => {
							laneState.addedDirs.remove(dir);
						}}
					/>

					<CodegenInput
						bind:this={inputRef}
						{projectId}
						{stackId}
						branchName={stableBranchName}
						value={initialPrompt || ''}
						loading={['running', 'compacting'].includes(status)}
						compacting={status === 'compacting'}
						{onChange}
						onSubmit={async (prompt) => {
							await onSubmit?.(prompt);
							setTimeout(() => {
								virtualList?.scrollToBottom();
							}, 100);
						}}
						{onAbort}
						onCancel={onclose}
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
				</div>
			{/if}
		{/snippet}
	</ReduxResult>
</div>

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
			{#snippet children(_promptTemplates, { projectId: _projectId })}
				{#each parsedTemplates as template}
					{@const displayName = template.parsed.name || template.fileName}

					<ContextMenuItem
						label={displayName}
						emoji={template.parsed.emoji || undefined}
						icon={template.parsed.emoji ? undefined : 'script'}
						onclick={() => {
							insertTemplate(template.parsed.content);
						}}
					/>
				{/each}
			{/snippet}
		</ReduxResult>
	</ContextMenuSection>
	<ContextMenuSection>
		<ContextMenuItem
			label="Edit templates…"
			icon="edit"
			onclick={() => {
				promptConfigModal?.show();
				templateContextMenu?.close();
			}}
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

<style lang="postcss">
	.chat {
		container-name: chat;
		container-type: inline-size;
		display: flex;
		position: relative;
		flex: 1;
		flex-direction: column;
		width: 100%;
		height: 100%;
		overflow: hidden;
	}

	.chat-container {
		--message-max-width: 840px;
		display: flex;
		position: relative;
		flex: 1;
		flex-grow: 1;
		flex-direction: column;
		width: 100%;
		height: 100%;
		min-height: 10rem;
		overflow: hidden;
	}
	.chat-view__placeholder {
		display: flex;
		flex: 1;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		padding: 28px;
	}

	.dialog-wrapper {
		display: flex;
		position: relative;
		flex-shrink: 0;
		flex-direction: column;
		width: 100%;
		padding: 16px;
		gap: 8px;
		border-top: 1px solid var(--clr-border-2);
	}

	.no-agent-placeholder {
		display: flex;
		flex-direction: column;
		justify-content: space-between;
		height: 100%;
		margin: 40px 0;
		padding: 0 40px;
	}

	.no-agent-placeholder__content {
		display: flex;
		flex-direction: column;
		justify-content: center;
		height: 100%;
		margin-bottom: 32px;
		gap: 18px;
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

	.chat-skeleton {
		display: flex;
		flex: 1;
		flex-direction: column;
		padding: 20px;
		gap: 20px;
	}

	.chat-skeleton__user {
		display: flex;
		flex-direction: column;
		align-items: flex-end;
		justify-content: center;
	}

	.chat-skeleton__assistant {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		max-width: 80%;
		gap: 8px;
	}

	.input-skeleton {
		display: flex;
		flex-direction: column;
		width: 100%;
		padding: 12px;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
	}

	.input-skeleton__text {
		width: 100%;
		padding-bottom: 30px;
	}

	.input-skeleton__actions {
		display: flex;
		justify-content: space-between;
		padding-top: 12px;
		border-top: 1px solid var(--clr-border-3);
	}
</style>
