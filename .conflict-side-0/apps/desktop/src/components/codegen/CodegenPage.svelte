<script lang="ts">
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
	import CodegenMessages from '$components/codegen/CodegenMessages.svelte';
	import CodegenSidebar from '$components/codegen/CodegenSidebar.svelte';
	import CodegenSidebarEntry from '$components/codegen/CodegenSidebarEntry.svelte';
	import CodegenTodo from '$components/codegen/CodegenTodo.svelte';
	import ClaudeCheck from '$components/v3/ClaudeCheck.svelte';
	import appClickSvg from '$lib/assets/empty-state/app-click.svg?raw';
	import codegenSvg from '$lib/assets/empty-state/codegen.svg?raw';
	import emptyFolderSvg from '$lib/assets/empty-state/empty-folder.svg?raw';
	import filesAndChecksSvg from '$lib/assets/empty-state/files-and-checks.svg?raw';
	import vibecodingSvg from '$lib/assets/illustrations/vibecoding.svg?raw';
	import { useAvailabilityChecking } from '$lib/codegen/availabilityChecking.svelte';
	import { CLAUDE_CODE_SERVICE } from '$lib/codegen/claude';
	import { currentStatus, getTodos, lastInteractionTime, usageStats } from '$lib/codegen/messages';

	import { commitStatusLabel } from '$lib/commits/commit';
	import { isAiRule, type RuleFilter } from '$lib/rules/rule';
	import { RULES_SERVICE } from '$lib/rules/rulesService.svelte';
	import { createWorktreeSelection } from '$lib/selection/key';
	import { pushStatusToColor, pushStatusToIcon } from '$lib/stacks/stack';
	import { STACK_SERVICE } from '$lib/stacks/stackService.svelte';
	import { combineResults } from '$lib/state/helpers';
	import { UI_STATE } from '$lib/state/uiState.svelte';
	import { createBranchRef } from '$lib/utils/branch';
	import { inject } from '@gitbutler/core/context';
	import { Badge, Button, chipToasts, EmptyStatePlaceholder } from '@gitbutler/ui';
	import { getColorFromBranchType } from '@gitbutler/ui/utils/getColorFromBranchType';

	import type { ClaudeMessage, PermissionMode } from '$lib/codegen/types';

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
	const rulesService = inject(RULES_SERVICE);
	const uiState = inject(UI_STATE);

	const stacks = $derived(stackService.stacks(projectId));
	const claudeAvailable = $derived(claudeCodeService.checkAvailable(undefined));
	const workspaceRules = $derived(rulesService.workspaceRules(projectId));
	const hasExistingSessions = $derived.by(() => {
		const stackss = stacks.response ?? [];
		const aiRules = (workspaceRules.response ?? []).filter(isAiRule);
		return stackss.some((stack) =>
			aiRules.some((rule) => rule.action.subject.subject.target.subject === stack.id)
		);
	});

	let settingsModal: ClaudeCodeSettingsModal | undefined;

	const permissionModeOptions: { label: string; value: PermissionMode }[] = [
		{ label: 'Edit with permission', value: 'default' },
		{ label: 'Planning', value: 'plan' },
		{ label: 'Accept edits', value: 'acceptEdits' }
	];

	const projectState = uiState.project(projectId);
	const selectedBranch = $derived(projectState.selectedClaudeSession.current);
	const selectedPermissionMode = $derived(
		selectedBranch ? uiState.lane(selectedBranch.stackId).permissionMode.current : 'default'
	);
	const laneState = $derived(
		selectedBranch?.stackId ? uiState.lane(selectedBranch.stackId) : undefined
	);

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

	const events = $derived(
		claudeCodeService.messages({ projectId, stackId: selectedBranch?.stackId || '' })
	);

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
			<!-- It's difficult to start at bottom unless we re-render `CodegenMessages` -->
			{#key selectedBranch.head}
				<CodegenMessages
					isWorkspace={false}
					{projectId}
					stackId={selectedBranch.stackId}
					branchName={selectedBranch.head}
				/>
			{/key}

			{@render rightSidebar(events.response || [])}
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
							notScrollable={changes.length === 0}
							persistId={`codegen-changed-files-drawer-${projectId}-${selectedBranch.stackId}`}
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
				<Drawer
					persistId="codegen-todos-drawer-{projectId}-{selectedBranch.stackId}"
					defaultCollapsed={false}
					noshrink
					bottomBorder={addedDirs.length > 0}
				>
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
				<Drawer
					persistId="codegen-added-dirs-drawer-{projectId}-{selectedBranch.stackId}"
					defaultCollapsed={false}
					noshrink
				>
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
		{@const isActive = claudeCodeService.isStackActive(projectId, stackId)}
		{@const rule = rulesService.aiRuleForStack({ projectId, stackId })}

		<ReduxResult
			result={combineResults(
				branch.result,
				commits.result,
				branchDetails.result,
				events.result,
				isActive.result,
				rule.result
			)}
			{projectId}
			{stackId}
		>
			{#snippet children(
				[branch, commits, branchDetailsData, events, isActive, rule],
				{ projectId: _projectId, stackId }
			)}
				{@const usage = usageStats(events)}
				{@const iconName = pushStatusToIcon(branchDetailsData.pushStatus)}
				{@const lineColor = getColorFromBranchType(pushStatusToColor(branchDetailsData.pushStatus))}

				<!-- Get session details if rule exists -->
				{#if rule}
					{@const sessionId = (rule.filters[0] as RuleFilter & { type: 'claudeCodeSessionId' })
						?.subject}
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
							gerritReviewUrl={commit.gerritReviewUrl}
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

<ClaudeCodeSettingsModal bind:this={settingsModal} onClose={() => {}} />

<CreateBranchModal bind:this={createBranchModal} {projectId} stackId={selectedBranch?.stackId} />

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
