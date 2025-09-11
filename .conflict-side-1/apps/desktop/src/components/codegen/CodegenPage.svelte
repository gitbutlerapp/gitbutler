<script lang="ts">
	import { goto } from '$app/navigation';
	import BranchHeaderIcon from '$components/BranchHeaderIcon.svelte';
	import CommitRow from '$components/CommitRow.svelte';
	import ConfigurableScrollableContainer from '$components/ConfigurableScrollableContainer.svelte';
	import CreateBranchModal from '$components/CreateBranchModal.svelte';
	import Drawer from '$components/Drawer.svelte';
	import FileList from '$components/FileList.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import Resizer from '$components/Resizer.svelte';
	import ClaudeCodeSettingsModal from '$components/codegen/ClaudeCodeSettingsModal.svelte';
	import CodegenChatLayout from '$components/codegen/CodegenChatLayout.svelte';
	import CodegenClaudeMessage from '$components/codegen/CodegenClaudeMessage.svelte';
	import CodegenInput from '$components/codegen/CodegenInput.svelte';
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
	import { useAvailabilityChecking } from '$lib/codegen/availabilityChecking.svelte';
	import { CLAUDE_CODE_SERVICE } from '$lib/codegen/claude';
	import {
		currentStatus,
		formatMessages,
		getTodos,
		lastInteractionTime,
		lastUserMessageSentAt,
		userFeedbackStatus,
		usageStats
	} from '$lib/codegen/messages';
	import { commitStatusLabel } from '$lib/commits/commit';
	import { vscodePath } from '$lib/project/project';
	import { PROJECTS_SERVICE } from '$lib/project/projectsService';
	import { workspacePath } from '$lib/routes/routes.svelte';
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
		EmptyStatePlaceholder,
		Modal
	} from '@gitbutler/ui';
	import { getColorFromBranchType } from '@gitbutler/ui/utils/getColorFromBranchType';
	import type { ClaudeMessage, ThinkingLevel, ModelType } from '$lib/codegen/types';
	import type { RuleFilter } from '$lib/rules/rule';

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

	const stacks = $derived(stackService.stacks(projectId));
	const permissionRequests = $derived(claudeCodeService.permissionRequests({ projectId }));
	const claudeAvailable = $derived(claudeCodeService.checkAvailable(undefined));
	const [sendClaudeMessage] = claudeCodeService.sendMessage;

	let settingsModal: ClaudeCodeSettingsModal | undefined;
	let clearContextModal = $state<Modal>();
	let modelContextMenu = $state<ContextMenu>();
	let modelTrigger = $state<HTMLButtonElement>();
	let thinkingModeContextMenu = $state<ContextMenu>();
	let thinkingModeTrigger = $state<HTMLButtonElement>();
	let templateContextMenu = $state<ContextMenu>();
	let templateTrigger = $state<HTMLButtonElement>();

	const modelOptions: { label: string; value: ModelType }[] = [
		{ label: 'Sonnet', value: 'sonnet' },
		{ label: 'Sonnet 1m', value: 'sonnet[1m]' },
		{ label: 'Opus', value: 'opus' },
		{ label: 'Opus Planning', value: 'opusplan' }
	];

	const thinkingLevels: ThinkingLevel[] = ['normal', 'think', 'megaThink', 'ultraThink'];

	const promptTemplates = $derived(claudeCodeService.promptTemplates(undefined));

	const projectState = uiState.project(projectId);
	const selectedBranch = $derived(projectState.selectedClaudeSession.current);
	const selectedThinkingLevel = $derived(projectState.thinkingLevel.current);
	const selectedModel = $derived(projectState.selectedModel.current);

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
		if (stacks.current.data) {
			if (selectedBranch) {
				// Make sure the current selection is valid
				const branchFound = stacks.current.data.some(
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
		if (!stacks.current.data) return;

		const firstStack = stacks.current.data[0];
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
				model: selectedModel
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

	function thinkingLevelToUiLabel(level: ThinkingLevel): string {
		switch (level) {
			case 'normal':
				return 'Normal';
			case 'think':
				return 'Think';
			case 'megaThink':
				return 'Mega Think';
			case 'ultraThink':
				return 'Ultra Think';
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
	const isStackActiveResult = $derived(
		selectedBranch ? claudeCodeService.isStackActive(projectId, selectedBranch.stackId) : undefined
	);
	const isStackActive = $derived(isStackActiveResult?.current?.data || false);

	// Check if there are rules to delete for the current session
	const rules = $derived(rulesService.workspaceRules(projectId));
	const hasRulesToClear = $derived(() => {
		if (!events?.current.data || !rules.current.data) return false;

		const sessionId = getCurrentSessionId(events.current.data);
		if (!sessionId) return false;

		return rules.current.data.some((rule) =>
			rule.filters.some(
				(filter) => filter.type === 'claudeCodeSessionId' && filter.subject === sessionId
			)
		);
	});

	let rightSidebarRef = $state<HTMLDivElement>();
	let createBranchModal = $state<CreateBranchModal>();
	let chatLayout = $state<CodegenChatLayout>();

	// Auto-scroll when new messages are added or branch changes
	$effect(() => {
		if (events?.current.data) {
			setTimeout(() => {
				chatLayout?.scrollToBottom();
			}, 50);
		}
	});
</script>

<div class="page">
	<ReduxResult result={claudeAvailable.current} {projectId}>
		{#snippet children(claudeAvailable, { projectId })}
			{#if claudeAvailable.status === 'available'}
				{@render main({ projectId })}
			{:else}
				{@render claudeNotAvailable()}
			{/if}
		{/snippet}
	</ReduxResult>
</div>

{#snippet main({ projectId }: { projectId: string })}
	<CodegenSidebar>
		{#snippet actions()}
			<Button
				kind="outline"
				size="tag"
				icon="plus-small"
				reversedDirection
				onclick={() => createBranchModal?.show()}>Add new</Button
			>
			<Button kind="ghost" icon="mixer" size="tag" onclick={() => settingsModal?.show()} />
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
					events?.current,
					permissionRequests.current,
					selectedBranchDetails.current
				)}
				{projectId}
			>
				{#snippet children(
					[events, permissionRequests, branchDetailsData],
					{ projectId: _projectId }
				)}
					{@const formattedMessages = formatMessages(events, permissionRequests, isStackActive)}
					{@const lastUserMessageSent = lastUserMessageSentAt(events)}
					{@const iconName = pushStatusToIcon(branchDetailsData.pushStatus)}
					{@const lineColor = getColorFromBranchType(
						pushStatusToColor(branchDetailsData.pushStatus)
					)}

					<CodegenChatLayout bind:this={chatLayout} branchName={selectedBranch.head}>
						{#snippet branchIcon()}
							<BranchHeaderIcon {iconName} color={lineColor} />
						{/snippet}
						{#snippet workspaceActions()}
							<Button kind="outline" size="tag" icon="workbench-small" onclick={showInWorkspace}
								>Show in workspace</Button
							>
							<Button
								kind="outline"
								icon="open-editor-small"
								size="tag"
								tooltip="Open in editor
							"
								onclick={openInEditor}
							/>
						{/snippet}
						{#snippet contextActions()}
							<Button
								disabled={!hasRulesToClear || formattedMessages.length === 0}
								kind="outline"
								size="tag"
								icon="clear-small"
								reversedDirection
								onclick={clearContextAndRules}
							>
								Clear context
							</Button>
						{/snippet}
						{#snippet messages()}
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
								{#each formattedMessages as message}
									<CodegenClaudeMessage
										{message}
										{onApproval}
										{onRejection}
										userAvatarUrl={$user?.picture}
									/>
								{/each}
							{/if}

							{#if currentStatus(events, isStackActive) === 'running' && lastUserMessageSent}
								{@const status = userFeedbackStatus(formattedMessages)}
								{#if status.waitingForFeedback}
									<CodegenServiceMessageUseTool toolCall={status.toolCall} />
								{:else}
									<CodegenServiceMessageThinking
										{lastUserMessageSent}
										msSpentWaiting={status.msSpentWaiting}
									/>
								{/if}
							{/if}
						{/snippet}

						{#snippet input()}
							<CodegenInput
								value={prompt}
								onChange={(prompt) => setPrompt(prompt)}
								loading={currentStatus(events, isStackActive) === 'running'}
								onsubmit={sendMessage}
								{onAbort}
							>
								{#snippet actions()}
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
											icon="brain"
											reversedDirection
											onclick={() => thinkingModeContextMenu?.toggle()}
											tooltip="Thinking Mode"
											children={selectedThinkingLevel === 'normal' ? undefined : thinkingBtnText}
										/>
									</div>

									<Button
										bind:el={modelTrigger}
										kind="ghost"
										icon="chevron-down"
										shrinkable
										onclick={() => modelContextMenu?.toggle()}
									>
										{modelOptions.find((a) => a.value === selectedModel)?.label}
									</Button>
								{/snippet}
							</CodegenInput>
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
	<div class="right-sidebar" bind:this={rightSidebarRef}>
		{#if !branchChanges || !selectedBranch || (branchChanges.current?.data && branchChanges.current.data.changes.length === 0 && getTodos(events).length === 0)}
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
				<ReduxResult result={branchChanges.current} {projectId}>
					{#snippet children({ changes }, { projectId })}
						<Drawer
							bottomBorder={todos.length > 0}
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
		{/if}

		<Resizer
			direction="left"
			viewport={rightSidebarRef}
			defaultValue={24}
			minWidth={20}
			maxWidth={35}
			persistId="resizer-codegenRight"
		/>
	</div>
{/snippet}

{#snippet sidebarContent()}
	<ReduxResult result={stacks.current} {projectId}>
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
				branch.current,
				commits.current,
				branchDetails.current,
				events.current,
				sidebarIsStackActive.current,
				rule.current
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
						<ReduxResult result={sessionDetails.current} {projectId}>
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
		<div class="not-available-form">
			<ClaudeCheck
				claudeExecutable={claudeExecutable.current}
				recheckedAvailability={recheckedAvailability.current}
				onUpdateExecutable={updateClaudeExecutable}
				onCheckAvailability={checkClaudeAvailability}
			/>
		</div>
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
			<ContextMenuItem label={option.label} onclick={() => selectModel(option.value)} />
		{/each}
	</ContextMenuSection>
</ContextMenu>

<ContextMenu
	bind:this={thinkingModeContextMenu}
	leftClickTrigger={thinkingModeTrigger}
	align="start"
	side="top"
>
	<ContextMenuSection title="Thinking Mode">
		{#each thinkingLevels as level}
			<ContextMenuItem
				label={thinkingLevelToUiLabel(level)}
				onclick={() => selectThinkingLevel(level)}
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
	<ContextMenuSection title="Templates">
		<ReduxResult result={promptTemplates.current} {projectId}>
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
			label="Edit templates in {$userSettings.defaultCodeEditor.displayName}"
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
		--message-max-width: 520px;
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
		border-left: 1px solid var(--clr-border-2);
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

	.not-available-form {
		display: flex;
		flex-direction: column;
		align-items: center;
		padding: 24px;
		gap: 16px;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
		background-color: var(--clr-bg-2);
		box-shadow: var(--shadow-elevation-low);
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
</style>
