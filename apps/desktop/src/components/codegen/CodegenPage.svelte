<script lang="ts">
	import { goto } from '$app/navigation';
	import BranchHeaderIcon from '$components/BranchHeaderIcon.svelte';
	import CommitRow from '$components/CommitRow.svelte';
	import ConfigurableScrollableContainer from '$components/ConfigurableScrollableContainer.svelte';
	import CreateBranchModal from '$components/CreateBranchModal.svelte';
	import DecorativeSplitView from '$components/DecorativeSplitView.svelte';
	import Drawer from '$components/Drawer.svelte';
	import FileList from '$components/FileList.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import Resizer from '$components/Resizer.svelte';
	import ClaudeCodeSettingsModal from '$components/codegen/ClaudeCodeSettingsModal.svelte';
	import CodegenChatClaudeNotAvaliableBanner from '$components/codegen/CodegenChatClaudeNotAvaliableBanner.svelte';
	import CodegenChatLayout from '$components/codegen/CodegenChatLayout.svelte';
	import CodegenClaudeMessage from '$components/codegen/CodegenClaudeMessage.svelte';
	import CodegenInput from '$components/codegen/CodegenInput.svelte';
	import CodegenMcpConfigModal from '$components/codegen/CodegenMcpConfigModal.svelte';
	import CodegenServiceMessageThinking from '$components/codegen/CodegenServiceMessageThinking.svelte';
	import CodegenServiceMessageUseTool from '$components/codegen/CodegenServiceMessageUseTool.svelte';
	import CodegenSidebar from '$components/codegen/CodegenSidebar.svelte';
	import CodegenSidebarEntry from '$components/codegen/CodegenSidebarEntry.svelte';
	import CodegenTodo from '$components/codegen/CodegenTodo.svelte';
	import ClaudeCheck from '$components/v3/ClaudeCheck.svelte';
	import appClickSvg from '$lib/assets/empty-state/app-click.svg?raw';
	import codegenSvg from '$lib/assets/empty-state/codegen.svg?raw';
	import emptyFolderSvg from '$lib/assets/empty-state/empty-folder.svg?raw';
	import filesAndChecksSvg from '$lib/assets/empty-state/files-and-checks.svg?raw';
	import laneNewSvg from '$lib/assets/empty-state/lane-new.svg?raw';
	import vibecodingSvg from '$lib/assets/illustrations/vibecoding.svg?raw';
	import { useAvailabilityChecking } from '$lib/codegen/availabilityChecking.svelte';
	import { CLAUDE_CODE_SERVICE } from '$lib/codegen/claude';
	import {
		currentStatus,
		formatMessages,
		getTodos,
		lastInteractionTime,
		thinkingOrCompactingStartedAt,
		userFeedbackStatus,
		usageStats,
		reverseMessages
	} from '$lib/codegen/messages';
	import { commitStatusLabel } from '$lib/commits/commit';
	import { SETTINGS_SERVICE } from '$lib/config/appSettingsV2';
	import { vscodePath } from '$lib/project/project';
	import { PROJECTS_SERVICE } from '$lib/project/projectsService';
	import { workspacePath } from '$lib/routes/routes.svelte';
	import { isAiRule, type RuleFilter } from '$lib/rules/rule';
	import { RULES_SERVICE } from '$lib/rules/rulesService.svelte';
	import { createWorktreeSelection } from '$lib/selection/key';
	import { SETTINGS } from '$lib/settings/userSettings';
	import { CODEGEN_ANALYTICS } from '$lib/soup/codegenAnalytics';
	import { pushStatusToColor, pushStatusToIcon } from '$lib/stacks/stack';
	import { STACK_SERVICE } from '$lib/stacks/stackService.svelte';
	import { combineResults } from '$lib/state/helpers';
	import { UI_STATE } from '$lib/state/uiState.svelte';
	import { USER } from '$lib/user/user';
	import { createBranchRef } from '$lib/utils/branch';
	import { getEditorUri, URL_SERVICE } from '$lib/utils/url';
	import { inject } from '@gitbutler/core/context';
	import {
		Badge,
		Button,
		chipToasts,
		ContextMenu,
		ContextMenuItem,
		ContextMenuSection,
		DropdownButton,
		EmptyStatePlaceholder,
		Modal
	} from '@gitbutler/ui';
	import { getColorFromBranchType } from '@gitbutler/ui/utils/getColorFromBranchType';

	import type { ClaudeMessage, ThinkingLevel, ModelType, PermissionMode } from '$lib/codegen/types';

	type Props = {
		projectId: string;
	};
	const { projectId }: Props = $props();

	const {
		claudeExecutable,
		recheckedAvailability,
		checkClaudeAvailability,
		updateClaudeExecutable
	} = useAvailabilityChecking();

	const claudeCodeService = inject(CLAUDE_CODE_SERVICE);
	const stackService = inject(STACK_SERVICE);
	const projectsService = inject(PROJECTS_SERVICE);
	const rulesService = inject(RULES_SERVICE);
	const codegenAnalytics = inject(CODEGEN_ANALYTICS);
	const uiState = inject(UI_STATE);
	const user = inject(USER);
	const urlService = inject(URL_SERVICE);
	const userSettings = inject(SETTINGS);
	const settingsService = inject(SETTINGS_SERVICE);
	const claudeSettings = $derived($settingsService?.claude);

	const stacks = $derived(stackService.stacks(projectId));
	const permissionRequests = $derived(claudeCodeService.permissionRequests({ projectId }));
	const claudeAvailable = $derived(claudeCodeService.checkAvailable(undefined));
	const workspaceRules = $derived(rulesService.workspaceRules(projectId));
	const hasExistingSessions = $derived.by(() => {
		const stackss = stacks.response ?? [];
		const aiRules = (workspaceRules.response ?? []).filter(isAiRule);
		return stackss.some((stack) =>
			aiRules.some((rule) => rule.action.subject.subject.target.subject === stack.id)
		);
	});
	const [sendClaudeMessage] = claudeCodeService.sendMessage;
	const mcpConfig = $derived(claudeCodeService.mcpConfig({ projectId }));

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

	const modelOptions: { label: string; value: ModelType }[] = [
		{ label: 'Sonnet', value: 'sonnet' },
		{ label: 'Sonnet 1m', value: 'sonnet[1m]' },
		{ label: 'Opus', value: 'opus' },
		{ label: 'Opus Planning', value: 'opusplan' }
	];

	const thinkingLevels: ThinkingLevel[] = ['normal', 'think', 'megaThink', 'ultraThink'];

	const permissionModeOptions: { label: string; value: PermissionMode }[] = [
		{ label: 'Edit with permission', value: 'default' },
		{ label: 'Planning', value: 'plan' },
		{ label: 'Accept edits', value: 'acceptEdits' }
	];

	const promptTemplates = $derived(claudeCodeService.promptTemplates(undefined));

	const projectState = uiState.project(projectId);
	const selectedBranch = $derived(projectState.selectedClaudeSession.current);
	const selectedThinkingLevel = $derived(projectState.thinkingLevel.current);
	const selectedModel = $derived(projectState.selectedModel.current);
	const selectedPermissionMode = $derived(
		selectedBranch ? uiState.lane(selectedBranch.stackId).permissionMode.current : 'default'
	);
	const laneState = $derived(
		selectedBranch?.stackId ? uiState.lane(selectedBranch.stackId) : undefined
	);

	const prompt = $derived(
		selectedBranch ? uiState.lane(selectedBranch.stackId).prompt.current : ''
	);
	function setPrompt(prompt: string) {
		if (!selectedBranch) return;
		uiState.lane(selectedBranch.stackId).prompt.set(prompt);
	}

	// File list data
	const branchChanges = $derived(
		selectedBranch
			? stackService.branchChanges({
					projectId,
					stackId: selectedBranch.stackId,
					branch: createBranchRef(selectedBranch.head, undefined)
				})
			: undefined
	);
	const selectionId = $derived(createWorktreeSelection({ stackId: selectedBranch?.stackId }));

	$effect(() => {
		if (stacks.response) {
			if (selectedBranch) {
				// Make sure the current selection is valid
				const branchFound = stacks.response.some(
					(s) =>
						s.id === selectedBranch?.stackId && s.heads.some((h) => h.name === selectedBranch?.head)
				);
				if (!branchFound) {
					selectFirstBranch();
				}
			} else {
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

	async function sendMessage() {
		if (!selectedBranch) return;
		if (!prompt) return;

		if (prompt.startsWith('/compact')) {
			compactContext();
			return;
		}

		// Handle /add-dir command
		if (prompt.startsWith('/add-dir ')) {
			const path = prompt.slice('/add-dir '.length).trim();
			if (path) {
				const isValid = await claudeCodeService.verifyPath({ projectId, path });
				if (isValid) {
					laneState?.addedDirs.add(path);
					chipToasts.success(`Added directory: ${path}`);
				} else {
					chipToasts.error(`Invalid directory path: ${path}`);
				}
			}
			setPrompt('');
			return;
		}

		if (prompt.startsWith('/')) {
			chipToasts.warning('Slash commands are not yet supported');
			setPrompt('');
			return;
		}

		// Await analytics data before sending message
		const analyticsProperties = await codegenAnalytics.getCodegenProperties({
			projectId,
			stackId: selectedBranch.stackId,
			message: prompt,
			thinkingLevel: selectedThinkingLevel,
			model: selectedModel
		});

		const promise = sendClaudeMessage(
			{
				projectId,
				stackId: selectedBranch.stackId,
				message: prompt,
				thinkingLevel: selectedThinkingLevel,
				model: selectedModel,
				permissionMode: selectedPermissionMode,
				disabledMcpServers: uiState.lane(selectedBranch.stackId).disabledMcpServers.current,
				addDirs: laneState?.addedDirs.current || []
			},
			{ properties: analyticsProperties }
		);

		setPrompt('');
		await promise;
	}

	async function onApproval(id: string) {
		await claudeCodeService.updatePermissionRequest({ projectId, requestId: id, approval: true });
	}
	async function onRejection(id: string) {
		await claudeCodeService.updatePermissionRequest({ projectId, requestId: id, approval: false });
	}
	async function onAbort() {
		if (!selectedBranch) return;
		await claudeCodeService.cancelSession({ projectId, stackId: selectedBranch?.stackId });
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
		if (!selectedBranch) return;
		uiState.lane(selectedBranch.stackId).permissionMode.set(mode);
		permissionModeContextMenu?.close();
	}

	function cyclePermissionMode() {
		if (!selectedBranch) return;
		const currentIndex = permissionModeOptions.findIndex(
			(option) => option.value === selectedPermissionMode
		);
		const nextIndex = (currentIndex + 1) % permissionModeOptions.length;
		const nextMode = permissionModeOptions[nextIndex];
		if (nextMode) {
			uiState.lane(selectedBranch.stackId).permissionMode.set(nextMode.value);
		}
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

	function thinkingLevelToUiLabel(level: ThinkingLevel): string {
		switch (level) {
			case 'normal':
				return 'Normal';
			case 'think':
				return 'Think';
			case 'megaThink':
				return 'Mega think';
			case 'ultraThink':
				return 'Ultra think';
			default:
				return 'Normal';
		}
	}

	function insertTemplate(template: string) {
		setPrompt(prompt + (prompt ? '\n\n' : '') + template);
		templateContextMenu?.close();
	}

	async function configureTemplates() {
		templateContextMenu?.close();

		const templatesPath = await claudeCodeService.fetchPromptTemplatesPath(undefined);

		if (templatesPath) {
			const editorUri = getEditorUri({
				schemeId: $userSettings.defaultCodeEditor.schemeIdentifer,
				path: [templatesPath]
			});

			urlService.openExternalUrl(editorUri);
		}
	}

	function showInWorkspace() {
		if (!selectedBranch) return;
		goto(`${workspacePath(projectId)}?stackId=${selectedBranch.stackId}`);
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
		if (!selectedBranch) return;

		await claudeCodeService.compactHistory({
			projectId,
			stackId: selectedBranch.stackId
		});
	}

	let selectedContextAction = $state<'clear' | 'compact'>('compact');

	async function performClearContextAndRules() {
		if (!selectedBranch) return;

		const events = await claudeCodeService.fetchMessages({
			projectId,
			stackId: selectedBranch.stackId
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

	const events = $derived(
		claudeCodeService.messages({ projectId, stackId: selectedBranch?.stackId || '' })
	);
	const isStackActiveQuery = $derived(
		selectedBranch ? claudeCodeService.isStackActive(projectId, selectedBranch.stackId) : undefined
	);
	const isStackActive = $derived(isStackActiveQuery?.response || false);

	// Check if there are rules to delete for the current session
	const rules = $derived(rulesService.workspaceRules(projectId));
	const hasRulesToClear = $derived(() => {
		if (!events?.response || !rules.response) return false;

		const sessionId = getCurrentSessionId(events.response);
		if (!sessionId) return false;

		return rules.response.some((rule) =>
			rule.filters.some(
				(filter) => filter.type === 'claudeCodeSessionId' && filter.subject === sessionId
			)
		);
	});

	let rightSidebarRef = $state<HTMLDivElement>();
	let createBranchModal = $state<CreateBranchModal>();

	function handleKeydown(event: KeyboardEvent) {
		// Ignore if user is typing in an input or textarea
		if (event.target instanceof HTMLInputElement || event.target instanceof HTMLTextAreaElement) {
			return;
		}

		// Handle Shift+Tab to cycle permission mode
		if (event.key === 'p' && event.metaKey) {
			event.preventDefault();
			cyclePermissionMode();
		}
	}
</script>

<svelte:window onkeydown={handleKeydown} />

{#if selectedBranch?.stackId}
	<ReduxResult result={mcpConfig.result} {projectId} stackId={selectedBranch.stackId}>
		{#snippet children(mcpConfig, { projectId: _projectId, stackId: _stackId })}
			{@const laneState = uiState.lane(selectedBranch.stackId)}
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
{/if}

<div class="page">
	<ReduxResult result={claudeAvailable.result} {projectId}>
		{#snippet children(claudeAvailable, { projectId })}
			{#if claudeAvailable.status === 'available' || hasExistingSessions}
				{@render main({ projectId, available: claudeAvailable.status === 'available' })}
			{:else}
				{@render claudeNotAvailable()}
			{/if}
		{/snippet}
	</ReduxResult>
</div>

{#snippet main({ projectId, available }: { projectId: string; available?: boolean })}
	<CodegenSidebar>
		{#snippet actions()}
			<Button
				kind="outline"
				size="tag"
				icon="plus-small"
				reversedDirection
				onclick={() => createBranchModal?.show()}>Add new</Button
			>
			<div class="flex relative">
				{#if !available}
					<svg
						viewBox="0 0 10 10"
						fill="none"
						xmlns="http://www.w3.org/2000/svg"
						class="settings-warning-icon"
					>
						<path
							d="M3.70898 1.66797C4.28964 0.685942 5.71035 0.685941 6.29102 1.66797L9.28613 6.7373C9.87685 7.7372 9.15651 8.99999 7.99512 9H2.00488C0.843494 8.99999 0.123153 7.7372 0.713867 6.7373L3.70898 1.66797Z"
							fill="#E89910"
						/>
					</svg>
				{/if}

				<Button kind="outline" icon="mixer" size="tag" onclick={() => settingsModal?.show()} />
			</div>
		{/snippet}

		{#snippet content()}
			{@render sidebarContent()}
		{/snippet}
	</CodegenSidebar>

	<div class="chat-view">
		{#if selectedBranch}
			{@const selectedBranchDetails = stackService.branchDetails(
				projectId,
				selectedBranch.stackId,
				selectedBranch.head
			)}
			<ReduxResult
				result={combineResults(
					events?.result,
					permissionRequests.result,
					selectedBranchDetails.result
				)}
				{projectId}
			>
				{#snippet children(
					[events, permissionRequests, branchDetailsData],
					{ projectId: _projectId }
				)}
					{@const formattedMessages = formatMessages(events, permissionRequests, isStackActive)}
					{@const reversedFormatterdMessages = reverseMessages(formattedMessages)}
					{@const iconName = pushStatusToIcon(branchDetailsData.pushStatus)}
					{@const lineColor = getColorFromBranchType(
						pushStatusToColor(branchDetailsData.pushStatus)
					)}
					{@const enabledMcpServers = mcpConfig.result.data
						? Object.keys(mcpConfig.result.data.mcpServers).length -
							uiState.lane(selectedBranch.stackId).disabledMcpServers.current.length
						: 0}

					<CodegenChatLayout branchName={selectedBranch.head}>
						{#snippet branchIcon()}
							<BranchHeaderIcon {iconName} color={lineColor} />
						{/snippet}
						{#snippet workspaceActions()}
							<Button
								kind="outline"
								size="tag"
								icon="workbench-small"
								reversedDirection
								onclick={showInWorkspace}>Show in workspace</Button
							>
							<Button
								kind="outline"
								icon="open-editor-small"
								size="tag"
								tooltip="Open in editor"
								onclick={openInEditor}
								reversedDirection
							>
								Open in {$userSettings.defaultCodeEditor.displayName}
							</Button>
						{/snippet}
						{#snippet contextActions()}
							{@const stats = usageStats(events)}
							<Badge>Context utilization {(stats.contextUtilization * 100).toFixed(0)}%</Badge>
							<Button
								kind="outline"
								icon="mcp"
								reversedDirection
								onclick={() => mcpConfigModal?.open()}
								>MCP

								{#snippet badge()}
									<Badge kind="soft">{enabledMcpServers}</Badge>
								{/snippet}
							</Button>
							<DropdownButton
								disabled={!hasRulesToClear || formattedMessages.length === 0}
								loading={['running', 'compacting'].includes(currentStatus(events, isStackActive))}
								kind="outline"
								style="warning"
								menuSide="top"
								autoClose={true}
								onclick={() =>
									selectedContextAction === 'compact' ? compactContext() : clearContextAndRules()}
							>
								{selectedContextAction === 'compact' ? 'Compact context' : 'Clear context'}
								{#snippet contextMenuSlot()}
									<ContextMenuSection>
										<ContextMenuItem
											label="Compact context"
											icon="clear-small"
											onclick={() => (selectedContextAction = 'compact')}
										/>
										<ContextMenuItem
											label="Clear context"
											icon="clear-small"
											onclick={() => (selectedContextAction = 'clear')}
										/>
									</ContextMenuSection>
								{/snippet}
							</DropdownButton>
						{/snippet}
						{#snippet messages()}
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

							{#if formattedMessages.length === 0}
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
											Your branch is ready for AI-powered development. Describe what you'd like to
											build, and I'll generate the code to get you started.
										{/snippet}
									</EmptyStatePlaceholder>
								</div>
							{:else}
								{#each reversedFormatterdMessages as message}
									<CodegenClaudeMessage
										{message}
										{onApproval}
										{onRejection}
										userAvatarUrl={$user?.picture}
									/>
								{/each}
							{/if}
						{/snippet}

						{#snippet input()}
							{#if claudeAvailable.response?.status === 'available'}
								{@const status = currentStatus(events, isStackActive)}
								<CodegenInput
									value={prompt}
									onChange={(prompt) => setPrompt(prompt)}
									loading={['running', 'compacting'].includes(status)}
									compacting={status === 'compacting'}
									onsubmit={sendMessage}
									{onAbort}
									sessionKey={selectedBranch
										? `${selectedBranch.stackId}-${selectedBranch.head}`
										: undefined}
								>
									{#snippet actions()}
										{@const permissionModeLabel = permissionModeOptions.find(
											(a) => a.value === selectedPermissionMode
										)?.label}
										<div class="flex m-right-4 gap-4">
											<Button disabled kind="outline" icon="attachment" reversedDirection />
											<Button
												bind:el={templateTrigger}
												kind="outline"
												icon="script"
												tooltip="Insert template"
												onclick={(e) => templateContextMenu?.toggle(e)}
											/>
											<Button
												bind:el={thinkingModeTrigger}
												kind="outline"
												icon="thinking"
												reversedDirection
												onclick={() => thinkingModeContextMenu?.toggle()}
												tooltip="Thinking mode"
												children={selectedThinkingLevel === 'normal' ? undefined : thinkingBtnText}
											/>
											<Button
												bind:el={permissionModeTrigger}
												kind="outline"
												icon={getPermissionModeIcon(selectedPermissionMode)}
												shrinkable
												onclick={() => permissionModeContextMenu?.toggle()}
												tooltip={$settingsService?.claude.dangerouslyAllowAllPermissions
													? 'Permission modes disable when all permissions are allowed'
													: `Permission mode: ${permissionModeLabel}`}
												disabled={$settingsService?.claude.dangerouslyAllowAllPermissions}
											/>
										</div>

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
							{:else}
								<CodegenChatClaudeNotAvaliableBanner
									onSettingsBtnClick={() => settingsModal?.show()}
								/>
							{/if}
						{/snippet}
					</CodegenChatLayout>

					{@render rightSidebar(events)}
				{/snippet}
			</ReduxResult>
		{:else}
			<div class="chat-view__no-branches-placeholder">
				<EmptyStatePlaceholder image={codegenSvg} width={250} gap={24}>
					{#snippet caption()}
						Choose a branch from the sidebar
						<br />
						to begin your coding session
					{/snippet}
				</EmptyStatePlaceholder>
			</div>
		{/if}
	</div>
{/snippet}

{#snippet rightSidebar(events: ClaudeMessage[])}
	{@const addedDirs = laneState?.addedDirs.current || []}
	<div class="right-sidebar" bind:this={rightSidebarRef}>
		{#if !branchChanges || !selectedBranch || (branchChanges.response && branchChanges.response.changes.length === 0 && getTodos(events).length === 0 && addedDirs.length === 0)}
			<div class="right-sidebar__placeholder">
				<EmptyStatePlaceholder
					image={filesAndChecksSvg}
					width={250}
					topBottomPadding={0}
					bottomMargin={0}
				>
					{#snippet caption()}
						File changes, usage stats, and tasks will appear here during your coding session.
					{/snippet}
				</EmptyStatePlaceholder>
			</div>
		{:else}
			{@const todos = getTodos(events)}
			{#if branchChanges && selectedBranch}
				<ReduxResult result={branchChanges.result} {projectId}>
					{#snippet children({ changes }, { projectId })}
						<Drawer
							bottomBorder={todos.length > 0 || addedDirs.length > 0}
							grow
							defaultCollapsed={todos.length > 0}
							notFoldable
							notScrollable={changes.length === 0}
						>
							{#snippet header()}
								<h4 class="text-14 text-semibold truncate">Changed files</h4>
								<Badge>{changes.length}</Badge>
							{/snippet}

							{#if changes.length > 0}
								<FileList
									{projectId}
									stackId={selectedBranch.stackId}
									{changes}
									listMode="list"
									{selectionId}
									showCheckboxes={false}
									draggableFiles={false}
									hideLastFileBorder={true}
									allowUnselect={false}
								/>
							{:else}
								<div class="right-sidebar__changes-placeholder">
									<EmptyStatePlaceholder image={emptyFolderSvg} width={180} gap={4}>
										{#snippet caption()}
											No changes yet
										{/snippet}
									</EmptyStatePlaceholder>
								</div>
							{/if}
						</Drawer>
					{/snippet}
				</ReduxResult>
			{/if}

			{#if todos.length > 0}
				<Drawer defaultCollapsed={false} noshrink>
					{#snippet header()}
						<h4 class="text-14 text-semibold truncate">Todos</h4>
						<Badge>{todos.length}</Badge>
					{/snippet}

					<div class="right-sidebar-list">
						{#each todos as todo}
							<CodegenTodo {todo} />
						{/each}
					</div>
				</Drawer>
			{/if}

			{#if addedDirs.length > 0}
				<Drawer defaultCollapsed={false} noshrink>
					{#snippet header()}
						<h4 class="text-14 text-semibold truncate">Added Directories</h4>
						<Badge>{addedDirs.length}</Badge>
					{/snippet}

					<div class="right-sidebar-list right-sidebar-list--small-gap">
						{#each addedDirs as dir}
							<div class="added-dir-item">
								<span class="text-13 grow-1">{dir}</span>
								<Button
									kind="ghost"
									icon="bin"
									shrinkable
									onclick={() => {
										if (selectedBranch) {
											uiState.lane(selectedBranch.stackId).addedDirs.remove(dir);
											chipToasts.success(`Removed directory: ${dir}`);
										}
									}}
									tooltip="Remove directory"
								/>
							</div>
						{/each}
					</div>
				</Drawer>
			{/if}
		{/if}

		<Resizer
			direction="left"
			viewport={rightSidebarRef}
			showBorder
			defaultValue={24}
			minWidth={20}
			maxWidth={35}
			persistId="resizer-codegenRight"
		/>
	</div>
{/snippet}

{#snippet sidebarContent()}
	<ReduxResult result={stacks.result} {projectId}>
		{#snippet children(stacks, { projectId })}
			{#if stacks.length === 0}
				<div class="sidebar_placeholder">
					<EmptyStatePlaceholder image={appClickSvg} width={200} bottomMargin={20}>
						{#snippet title()}
							Start your first session
						{/snippet}
						{#snippet caption()}
							Create your first branch to begin building with AI assistance
						{/snippet}
						{#snippet actions()}
							<Button kind="outline" icon="plus-small" onclick={() => createBranchModal?.show()}
								>Add new branch</Button
							>
						{/snippet}
					</EmptyStatePlaceholder>
				</div>
			{:else}
				<ConfigurableScrollableContainer>
					<div class="sidebar-content">
						{#each stacks as stack}
							{#if stack.id}
								{#each stack.heads as head, headIndex}
									{@render sidebarContentEntry(
										projectId,
										stack.id,
										head.name,
										headIndex,
										stack.heads.length,
										headIndex === 0
									)}
								{/each}
							{/if}
						{/each}
					</div>
				</ConfigurableScrollableContainer>
			{/if}
		{/snippet}
	</ReduxResult>
{/snippet}

{#snippet sidebarContentEntry(
	projectId: string,
	stackId: string,
	head: string,
	headIndex: number,
	totalHeads: number,
	isFirstBranch: boolean
)}
	{#if isFirstBranch}
		{@const branch = stackService.branchByName(projectId, stackId, head)}
		{@const commits = stackService.commits(projectId, stackId, head)}
		{@const branchDetails = stackService.branchDetails(projectId, stackId, head)}
		{@const events = claudeCodeService.messages({
			projectId,
			stackId
		})}
		{@const sidebarIsStackActive = claudeCodeService.isStackActive(projectId, stackId)}
		{@const rule = rulesService.aiRuleForStack({ projectId, stackId })}

		<ReduxResult
			result={combineResults(
				branch.result,
				commits.result,
				branchDetails.result,
				events.result,
				sidebarIsStackActive.result,
				rule.result
			)}
			{projectId}
			{stackId}
		>
			{#snippet children(
				[branch, commits, branchDetailsData, events, isActive, ruleData],
				{ projectId: _projectId, stackId }
			)}
				{@const usage = usageStats(events)}
				{@const iconName = pushStatusToIcon(branchDetailsData.pushStatus)}
				{@const lineColor = getColorFromBranchType(pushStatusToColor(branchDetailsData.pushStatus))}

				<!-- Get session details if rule exists -->
				{#if ruleData?.rule}
					{@const sessionId = (
						ruleData.rule.filters[0] as RuleFilter & { type: 'claudeCodeSessionId' }
					)?.subject}
					{#if sessionId}
						{@const sessionDetails = claudeCodeService.sessionDetails(projectId, sessionId)}
						<ReduxResult result={sessionDetails.result} {projectId}>
							{#snippet children(sessionDetailsData, { projectId: _projectId })}
								<CodegenSidebarEntry
									onclick={() => {
										projectState.selectedClaudeSession.set({ stackId, head: branch.name });
									}}
									selected={selectedBranch?.stackId === stackId &&
										selectedBranch?.head === branch.name}
									branchName={branch.name}
									status={currentStatus(events, isActive ?? false)}
									tokensUsed={usage.tokens}
									cost={usage.cost}
									commitCount={commits.length}
									lastInteractionTime={lastInteractionTime(events)}
									commits={commitsList}
									{totalHeads}
									sessionInGui={sessionDetailsData?.inGui}
								>
									{#snippet branchIcon()}
										<BranchHeaderIcon {iconName} color={lineColor} small />
									{/snippet}
								</CodegenSidebarEntry>
							{/snippet}
						</ReduxResult>
					{:else}
						<!-- No session ID in rule -->
						<CodegenSidebarEntry
							onclick={() => {
								projectState.selectedClaudeSession.set({ stackId, head: branch.name });
							}}
							selected={selectedBranch?.stackId === stackId && selectedBranch?.head === branch.name}
							branchName={branch.name}
							status={currentStatus(events, isActive ?? false)}
							tokensUsed={usage.tokens}
							cost={usage.cost}
							commitCount={commits.length}
							lastInteractionTime={lastInteractionTime(events)}
							commits={commitsList}
							{totalHeads}
						>
							{#snippet branchIcon()}
								<BranchHeaderIcon {iconName} color={lineColor} small />
							{/snippet}
						</CodegenSidebarEntry>
					{/if}
				{:else}
					<!-- No rule found -->
					<CodegenSidebarEntry
						onclick={() => {
							projectState.selectedClaudeSession.set({ stackId, head: branch.name });
						}}
						selected={selectedBranch?.stackId === stackId && selectedBranch?.head === branch.name}
						branchName={branch.name}
						status={currentStatus(events, isActive ?? false)}
						tokensUsed={usage.tokens}
						cost={usage.cost}
						commitCount={commits.length}
						lastInteractionTime={lastInteractionTime(events)}
						commits={commitsList}
						{totalHeads}
					>
						{#snippet branchIcon()}
							<BranchHeaderIcon {iconName} color={lineColor} small />
						{/snippet}
					</CodegenSidebarEntry>
				{/if}
				<!-- defining this here so it's name doesn't conflict with the
				variable commits -->
				{#snippet commitsList()}
					{@const lastBranch = headIndex === totalHeads - 1}
					{#each commits as commit, i}
						<CommitRow
							disabled
							disableCommitActions
							commitId={commit.id}
							commitMessage={commit.message}
							type={commit.state.type}
							diverged={commit.state.type === 'LocalAndRemote' &&
								commit.id !== commit.state.subject}
							hasConflicts={commit.hasConflicts}
							createdAt={commit.createdAt}
							branchName={branch.name}
							first={i === 0}
							lastCommit={i === commits.length - 1}
							{lastBranch}
							tooltip={commitStatusLabel(commit.state.type)}
						/>
					{/each}
				{/snippet}
			{/snippet}
		</ReduxResult>
	{/if}
{/snippet}

{#snippet claudeNotAvailable()}
	<div class="not-available">
		<DecorativeSplitView hideDetails img={vibecodingSvg}>
			<div class="not-available__content">
				<h1 class="text-serif-40">Set up <i>Claude Code</i></h1>
				<ClaudeCheck
					claudeExecutable={claudeExecutable.current}
					recheckedAvailability={recheckedAvailability.current}
					onUpdateExecutable={updateClaudeExecutable}
					onCheckAvailability={checkClaudeAvailability}
					showTitle={false}
				/>
			</div>
		</DecorativeSplitView>
	</div>
{/snippet}

{#snippet thinkingBtnText()}
	{thinkingLevelToUiLabel(selectedThinkingLevel)}
{/snippet}

<ClaudeCodeSettingsModal bind:this={settingsModal} onClose={() => {}} />

<CreateBranchModal bind:this={createBranchModal} {projectId} stackId={selectedBranch?.stackId} />

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

<ContextMenu bind:this={modelContextMenu} leftClickTrigger={modelTrigger} side="top" align="start">
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
				label={thinkingLevelToUiLabel(level)}
				selected={selectedThinkingLevel === level}
				onclick={() => selectThinkingLevel(level)}
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
				icon={getPermissionModeIcon(option.value)}
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
				{#each promptTemplates.templates as template}
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
			label="Edit in {$userSettings.defaultCodeEditor.displayName}"
			icon="open-editor"
			onclick={configureTemplates}
		/>
	</ContextMenuSection>
</ContextMenu>

<style lang="postcss">
	.page {
		display: flex;
		width: 100%;
		height: 100%;
		gap: 8px;

		/* SHARABLE */
		--message-max-width: 620px;
	}

	.chat-view {
		display: flex;
		flex: 1;
		height: 100%;
		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-l);
		background-color: var(--clr-bg-1);
	}

	.chat-view__placeholder,
	.chat-view__no-branches-placeholder {
		display: flex;
		flex: 1;
		align-items: center;
		justify-content: center;
		padding: 0 32px;
	}

	.chat-view__no-branches-placeholder {
		background-color: var(--clr-bg-2);
	}

	.right-sidebar {
		display: flex;
		position: relative;
		flex-direction: column;
		height: 100%;
	}

	.right-sidebar-list {
		display: flex;
		flex-direction: column;
		padding: 14px;
		gap: 12px;
	}

	.right-sidebar__placeholder {
		display: flex;
		flex: 1;
		align-items: center;
		justify-content: center;
		background-color: var(--clr-bg-2);
	}

	.right-sidebar__changes-placeholder {
		display: flex;
		flex: 1;
		align-items: center;
		justify-content: center;
		padding: 20px;
		background-color: var(--clr-bg-2);
	}

	/* NO CC AVAILABLE */
	.not-available {
		display: flex;
		flex: 1;
		align-items: center;
		justify-content: center;
		width: 100%;
		height: 100%;
	}

	.not-available__content {
		display: flex;
		flex-direction: column;
		width: 100%;
		max-width: 480px;
		margin: 0 auto;
		gap: 20px;
	}

	.sidebar-content {
		display: flex;
		position: relative;
		flex-direction: column;
		padding: 12px;
		gap: 8px;
	}

	.sidebar_placeholder {
		display: flex;
		flex: 1;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		padding: 0 16px;
		gap: 16px;
	}

	.settings-warning-icon {
		z-index: 1;
		position: absolute;
		top: -2px;
		right: -3px;
		width: 9px;
		height: 9px;
	}

	.added-dir-item {
		display: flex;
		align-items: center;
		justify-content: space-between;
	}

	.right-sidebar-list--small-gap {
		gap: 4px;
	}
</style>
