<script lang="ts">
	import ReduxResult from '$components/ReduxResult.svelte';
	import CodegenChatLayout from '$components/codegen/CodegenChatLayout.svelte';
	import CodegenClaudeMessage from '$components/codegen/CodegenClaudeMessage.svelte';
	import CodegenInput from '$components/codegen/CodegenInput.svelte';
	import CodegenRunningMessage from '$components/codegen/CodegenRunningMessage.svelte';
	import CodegenSidebar from '$components/codegen/CodegenSidebar.svelte';
	import CodegenSidebarEntry from '$components/codegen/CodegenSidebarEntry.svelte';
	import { CLAUDE_CODE_SERVICE } from '$lib/codegen/claude';
	import {
		currentStatus,
		formatMessages,
		lastUserMessageSentAt,
		usageStats
	} from '$lib/codegen/messages';
	import { STACK_SERVICE } from '$lib/stacks/stackService.svelte';
	import { combineResults } from '$lib/state/helpers';
	import { inject } from '@gitbutler/shared/context';
	import { Badge, Button } from '@gitbutler/ui';

	type Props = {
		projectId: string;
	};
	const { projectId }: Props = $props();

	const claudeCodeService = inject(CLAUDE_CODE_SERVICE);
	const stackService = inject(STACK_SERVICE);

	const stacks = $derived(stackService.stacks(projectId));

	let message = $state('');
	let selectedBranch = $state<{ stackId: string; head: string }>();

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
		} else {
			selectedBranch = undefined;
		}
	});

	function selectFirstBranch() {
		if (!stacks.current.data) return;

		const firstStack = stacks.current.data[0];
		const firstHead = firstStack?.heads[0];
		if (firstHead && firstStack.id) {
			selectedBranch = {
				stackId: firstStack.id,
				head: firstHead.name
			};
		} else {
			selectedBranch = undefined;
		}
	}

	async function sendMessage() {
		if (!selectedBranch) return;
		if (!message) return;
		const promise = claudeCodeService.sendMessage({
			projectId,
			stackId: selectedBranch.stackId,
			message
		});
		message = '';
		await promise;
	}

	const events = $derived(
		claudeCodeService.messages({ projectId, stackId: selectedBranch?.stackId || '' })
	);
</script>

<div class="page">
	<CodegenSidebar content={sidebarContent}>
		{#snippet actions()}
			<Button disabled kind="outline" icon="plus-small" size="tag">Create new</Button>
		{/snippet}
	</CodegenSidebar>

	<div class="content">
		{#if selectedBranch}
			<CodegenChatLayout branchName={selectedBranch.head}>
				{#snippet workspaceActions()}
					<Button disabled kind="outline" size="tag" icon="workbench" reversedDirection
						>Show in workspace</Button
					>
					<Button disabled kind="outline" size="tag" icon="chevron-down">Open in editor</Button>
				{/snippet}
				{#snippet contextActions()}
					<Badge kind="soft">69% used context</Badge>
					<Button disabled kind="outline" size="tag">Clear context</Button>
					<Button disabled kind="ghost" size="tag" icon="kebab" />
				{/snippet}
				{#snippet messages()}
					<ReduxResult result={events?.current} {projectId}>
						{#snippet children(events, { projectId: _projectId })}
							{#each formatMessages(events) as message}
								<CodegenClaudeMessage {message} />
							{/each}
							{@const lastUserMessageSent = lastUserMessageSentAt(events)}
							{#if currentStatus(events) === 'running' && lastUserMessageSent}
								<CodegenRunningMessage {lastUserMessageSent} />
							{/if}
						{/snippet}
					</ReduxResult>
				{/snippet}
				{#snippet input()}
					<ReduxResult result={events?.current} {projectId}>
						{#snippet children(events, { projectId: _projectId })}
							<CodegenInput
								bind:value={message}
								loading={currentStatus(events) === 'running'}
								onsubmit={sendMessage}
							>
								{#snippet actions()}
									<Button disabled kind="outline" icon="attachment" reversedDirection
										>Context</Button
									>
								{/snippet}
							</CodegenInput>
						{/snippet}
					</ReduxResult>
				{/snippet}
			</CodegenChatLayout>
		{/if}
	</div>
</div>

{#snippet sidebarContent()}
	<ReduxResult result={stacks.current} {projectId}>
		{#snippet children(stacks, { projectId })}
			{#each stacks as stack}
				{#if stack.id}
					{#each stack.heads as head}
						{@render sidebarContentEntry(projectId, stack.id, head.name)}
					{/each}
				{/if}
			{/each}
		{/snippet}
	</ReduxResult>
{/snippet}

{#snippet sidebarContentEntry(projectId: string, stackId: string, head: string)}
	{@const branch = stackService.branchByName(projectId, stackId, head)}
	{@const commits = stackService.commits(projectId, stackId, head)}
	{@const events = claudeCodeService.messages({
		projectId,
		stackId
	})}
	<ReduxResult
		result={combineResults(branch.current, commits.current, events.current)}
		{projectId}
		{stackId}
	>
		{#snippet children([branch, commits, events], { projectId: _projectId, stackId })}
			{@const usage = usageStats(events)}
			<CodegenSidebarEntry
				onclick={() => {
					selectedBranch = { stackId, head: branch.name };
				}}
				selected={selectedBranch?.stackId === stackId && selectedBranch?.head === branch.name}
				branchName={branch.name}
				status={currentStatus(events)}
				tokensUsed={usage.tokens}
				cost={usage.cost}
				commitCount={commits.length}
				commits={commitsList}
			/>
			<!-- defining this here so it's name doesn't conflict with the
			variable commits -->
			{#snippet commitsList()}
				<p>There are commits, I swear</p>
			{/snippet}
		{/snippet}
	</ReduxResult>
{/snippet}

<style lang="postcss">
	.page {
		display: flex;
		width: 100%;
		height: 100%;

		gap: 8px;
	}

	.content {
		/* TODO: This should be resizable */
		flex-grow: 1;
		height: 100%;

		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-l);
		background-color: var(--clr-bg-1);
	}
</style>
