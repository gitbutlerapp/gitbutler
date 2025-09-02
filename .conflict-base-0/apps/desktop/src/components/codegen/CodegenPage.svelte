<script lang="ts">
	import { goto } from '$app/navigation';
	import CommitRow from '$components/CommitRow.svelte';
	import CreateBranchModal from '$components/CreateBranchModal.svelte';
	import Drawer from '$components/Drawer.svelte';
	import FileList from '$components/FileList.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import Resizer from '$components/Resizer.svelte';
	import ClaudeCodeSettingsModal from '$components/codegen/ClaudeCodeSettingsModal.svelte';
	import CodegenChatLayout from '$components/codegen/CodegenChatLayout.svelte';
	import CodegenClaudeMessage from '$components/codegen/CodegenClaudeMessage.svelte';
	import CodegenInput from '$components/codegen/CodegenInput.svelte';
	import CodegenServiceMessage from '$components/codegen/CodegenServiceMessage.svelte';
	import CodegenSidebar from '$components/codegen/CodegenSidebar.svelte';
	import CodegenSidebarEntry from '$components/codegen/CodegenSidebarEntry.svelte';
	import CodegenSidebarEntryDisabled from '$components/codegen/CodegenSidebarEntryDisabled.svelte';
	import CodegenTodo from '$components/codegen/CodegenTodo.svelte';
	import CodegenUsageStat from '$components/codegen/CodegenUsageStat.svelte';
	import ClaudeCheck from '$components/v3/ClaudeCheck.svelte';
	import filesAndChecksSvg from '$lib/assets/empty-state/files-and-checks.svg?raw';
	import laneNewSvg from '$lib/assets/empty-state/lane-new.svg?raw';
	import { CLAUDE_CODE_SERVICE } from '$lib/codegen/claude';
	import {
		currentStatus,
		formatMessages,
		getTodos,
		lastInteractionTime,
		lastUserMessageSentAt,
		usageStats
	} from '$lib/codegen/messages';
	import { commitStatusLabel } from '$lib/commits/commit';
	import { SETTINGS_SERVICE } from '$lib/config/appSettingsV2';
	import { workspacePath } from '$lib/routes/routes.svelte';
	import { RULES_SERVICE } from '$lib/rules/rulesService.svelte';
	import { createWorktreeSelection } from '$lib/selection/key';
	import { SETTINGS } from '$lib/settings/userSettings';
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
		ContextMenu,
		ContextMenuItem,
		ContextMenuSection,
		EmptyStatePlaceholder,
		Modal
	} from '@gitbutler/ui';
	import { focusable } from '@gitbutler/ui/focus/focusable';
	import type { ClaudeMessage, ThinkingLevel, ModelType } from '$lib/codegen/types';

	type Props = {
		projectId: string;
	};
	const { projectId }: Props = $props();

	const claudeCodeService = inject(CLAUDE_CODE_SERVICE);
	const stackService = inject(STACK_SERVICE);
	const settingsService = inject(SETTINGS_SERVICE);
	const rulesService = inject(RULES_SERVICE);
	const uiState = inject(UI_STATE);
	const user = inject(USER);
	const urlService = inject(URL_SERVICE);
	const userSettings = inject(SETTINGS);

	const stacks = $derived(stackService.stacks(projectId));
	const permissionRequests = $derived(claudeCodeService.permissionRequests({ projectId }));
	const claudeAvailable = $derived(claudeCodeService.checkAvailable(undefined));
	const settingsStore = settingsService.appSettings;

	let message = $state('');
	let claudeExecutable = $derived($settingsStore?.claude.executable || 'claude');
	let updatingExecutable = $state(false);
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
		if (!message) return;
		const promise = claudeCodeService.sendMessage({
			projectId,
			stackId: selectedBranch.stackId,
			message,
			thinkingLevel: selectedThinkingLevel,
			model: selectedModel
		});
		message = '';
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

	let recheckedAvailability = $state<'recheck-failed' | 'recheck-succeeded'>();
	async function checkClaudeAvailability() {
		const recheck = await claudeCodeService.fetchCheckAvailable(undefined, { forceRefetch: true });
		if (recheck) {
			recheckedAvailability = 'recheck-succeeded';
		} else {
			recheckedAvailability = 'recheck-failed';
		}
	}

	async function updateClaudeExecutable(value: string) {
		if (updatingExecutable) return;

		claudeExecutable = value;
		recheckedAvailability = undefined;
		await settingsService.updateClaude({ executable: value });
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
		message = message + (message ? '\n\n' : '') + template;
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

	// Auto-scroll when new messages are added
	$effect(() => {
		if (events?.current.data) {
			setTimeout(() => {
				chatLayout?.scrollToBottom();
			}, 50);
		}
	});
</script>

<div class="page" use:focusable>
	<ReduxResult result={claudeAvailable.current} {projectId}>
		{#snippet children(claudeAvailable, { projectId })}
			{#if claudeAvailable}
				{@render main({ projectId })}
			{:else}
				{@render claudeNotAvailable()}
			{/if}
		{/snippet}
	</ReduxResult>
</div>

{#snippet main({ projectId }: { projectId: string })}
	<CodegenSidebar content={sidebarContent}>
		{#snippet actions()}
			<Button
				kind="outline"
				size="tag"
				icon="plus-small"
				reversedDirection
				onclick={() => createBranchModal?.show()}>Add new</Button
			>
			<Button kind="ghost" icon="settings" size="tag" onclick={() => settingsModal?.show()} />
		{/snippet}
	</CodegenSidebar>

	<div class="chat-view">
		{#if selectedBranch}
			<ReduxResult result={combineResults(events?.current, permissionRequests.current)} {projectId}>
				{#snippet children([events, permissionRequests], { projectId: _projectId })}
					{@const formattedMessages = formatMessages(events, permissionRequests, isStackActive)}
					{@const lastUserMessageSent = lastUserMessageSentAt(events)}

					<CodegenChatLayout bind:this={chatLayout} branchName={selectedBranch.head}>
						{#snippet workspaceActions()}
							<Button
								kind="outline"
								size="tag"
								icon="workbench"
								reversedDirection
								onclick={showInWorkspace}>Show in workspace</Button
							>
							<Button disabled kind="outline" size="tag" icon="chevron-down">Open in editor</Button>
						{/snippet}
						{#snippet contextActions()}
							<Badge kind="soft" size="tag">69% used context</Badge>
							<Button
								disabled={!hasRulesToClear}
								kind="outline"
								size="tag"
								icon="clear-small"
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
											Ready to code with AI
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
								<CodegenServiceMessage {lastUserMessageSent} />
							{/if}
						{/snippet}

						{#snippet input()}
							<CodegenInput
								bind:value={message}
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

					{@render rightSidebar(events, formattedMessages.length > 0)}
				{/snippet}
			</ReduxResult>
		{/if}
	</div>
{/snippet}

{#snippet rightSidebar(events: ClaudeMessage[], hasMessages: boolean)}
	<div class="right-sidebar" bind:this={rightSidebarRef}>
		{#if !hasMessages}
			<div class="right-sidebar__placeholder">
				<EmptyStatePlaceholder
					image={filesAndChecksSvg}
					width={240}
					topBottomPadding={0}
					bottomMargin={0}
				>
					{#snippet caption()}
						Once you begin a conversation, you'll see todos and usage statistics here.
					{/snippet}
				</EmptyStatePlaceholder>
			</div>
		{:else}
			<Drawer
				title="Todos"
				bottomBorder
				resizer={{
					persistId: 'codegen-todos',
					direction: 'down',
					minHeight: 8,
					maxHeight: 32,
					defaultValue: 16
				}}
			>
				{@const todos = getTodos(events)}
				<div class="right-sidebar-list">
					{#each todos as todo}
						<CodegenTodo {todo} />
					{/each}
				</div>
			</Drawer>

			{#if branchChanges && selectedBranch}
				<ReduxResult result={branchChanges.current} {projectId}>
					{#snippet children({ changes }, { projectId })}
						<Drawer
							title="Files"
							bottomBorder
							resizer={{
								persistId: 'codegen-files',
								direction: 'down',
								minHeight: 8,
								maxHeight: 38,
								defaultValue: 16
							}}
						>
							<div class="file-list-container">
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
							</div>
						</Drawer>
					{/snippet}
				</ReduxResult>
			{/if}

			<Drawer title="Usage">
				{@const usage = usageStats(events)}
				<div class="right-sidebar-list">
					<CodegenUsageStat label="Tokens used:" value={usage.tokens.toString()} />
					<CodegenUsageStat
						label="Total cost:"
						value={`$${usage.cost.toFixed(2)}`}
						valueSize="large"
					/>
				</div>
			</Drawer>
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
		{@const events = claudeCodeService.messages({
			projectId,
			stackId
		})}
		{@const sidebarIsStackActive = claudeCodeService.isStackActive(projectId, stackId)}
		<ReduxResult
			result={combineResults(
				branch.current,
				commits.current,
				events.current,
				sidebarIsStackActive.current
			)}
			{projectId}
			{stackId}
		>
			{#snippet children([branch, commits, events, isActive], { projectId: _projectId, stackId })}
				{@const usage = usageStats(events)}
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
				/>
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
	{:else}
		{@const branch = stackService.branchByName(projectId, stackId, head)}
		<ReduxResult result={branch.current} {projectId} {stackId}>
			{#snippet children(branch, { projectId: _projectId, stackId: _stackId })}
				<CodegenSidebarEntryDisabled branchName={branch.name} />
			{/snippet}
		</ReduxResult>
	{/if}
{/snippet}

{#snippet claudeNotAvailable()}
	<div class="not-available">
		<div class="not-available-form">
			<ClaudeCheck
				{claudeExecutable}
				{recheckedAvailability}
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

<ContextMenu bind:this={modelContextMenu} leftClickTrigger={modelTrigger} side="top">
	<ContextMenuSection>
		{#each modelOptions as option}
			<ContextMenuItem label={option.label} onclick={() => selectModel(option.value)} />
		{/each}
	</ContextMenuSection>
</ContextMenu>

<ContextMenu bind:this={thinkingModeContextMenu} leftClickTrigger={thinkingModeTrigger} side="top">
	<ContextMenuSection title="Thinking Mode">
		{#each thinkingLevels as level}
			<ContextMenuItem
				label={thinkingLevelToUiLabel(level)}
				onclick={() => selectThinkingLevel(level)}
			/>
		{/each}
	</ContextMenuSection>
</ContextMenu>

<ContextMenu bind:this={templateContextMenu} leftClickTrigger={templateTrigger} side="top">
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

	.chat-view__placeholder {
		display: flex;
		flex: 1;
		align-items: center;
		justify-content: center;
		padding: 0 32px;
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
		flex-direction: column;
		background-color: var(--clr-bg-2);
	}
	.file-list-container {
		display: flex;
		flex-direction: column;
		max-height: 200px;
		overflow-y: auto;
	}

	.right-sidebar__placeholder {
		display: flex;
		flex: 1;
		align-items: center;
		justify-content: center;
		background-color: var(--clr-bg-2);
	}

	/* NO CC AVAILABLE */
	.not-available {
		display: flex;
		flex: 1;
		align-items: center;
		justify-content: center;
		height: 100%;
	}
	.not-available-form {
		display: flex;
		flex-direction: column;
		max-width: 400px;
		padding: 20px;
		overflow: hidden;
		gap: 12px;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-l);
		background-color: var(--clr-bg-1);
	}
</style>
