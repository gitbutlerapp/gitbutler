<script lang="ts">
	import { getToolCallIcon, parseToolCall, type ToolCall } from '$lib/ai/tool';
	import { UI_STATE } from '$lib/state/uiState.svelte';
	import { inject } from '@gitbutler/core/context';
	import { Button, Icon } from '@gitbutler/ui';

	import type { WorkflowKind } from '$lib/actions/types';

	interface BaseProps {
		projectId: string;
		type: 'workflow' | 'tool-call';
	}

	interface WorkflowProps extends BaseProps {
		type: 'workflow';
		kind: WorkflowKind;
	}

	interface ToolCallProps extends BaseProps {
		type: 'tool-call';
		toolCall: ToolCall;
	}

	type Props = WorkflowProps | ToolCallProps;

	const { projectId, ...rest }: Props = $props();
	const uiState = inject(UI_STATE);

	let isExpanded = $state(false);

	function selectCommit(stackId: string, branchName: string, commitId: string) {
		const projectState = uiState.project(projectId);
		const laneState = uiState.lane(stackId);
		laneState.selection.set({
			branchName,
			commitId
		});
		projectState.stackId.set(stackId);
	}

	function selectBranch(stackId: string, branchName: string) {
		const projectState = uiState.project(projectId);
		const laneState = uiState.lane(stackId);
		laneState.selection.set({
			branchName
		});
		projectState.stackId.set(stackId);
	}
</script>

{#snippet code(value: unknown)}
	<div class="code-block">
		<pre class="text-11">{JSON.stringify(value, null, 2)}</pre>
	</div>
{/snippet}

{#if rest.type === 'workflow'}
	{@const kind = rest.kind}
	{#if kind.type === 'reword'}
		<div class="text-13">
			{#if kind.subject}
				{@const workflow = kind.subject}
				<div class="operations">
					<div class="operation-row text-13">
						<div class="operation-icon">
							<Icon name="commit" />
						</div>

						<div class="operation-content">
							<p class="operation__title">
								Updated commit:
								<button
									class="operation__commit-sha"
									type="button"
									onclick={() =>
										selectCommit(workflow.stackId, workflow.branchName, workflow.commitId)}
								>
									<span>
										{workflow.commitId.substring(0, 7)}
									</span>
								</button>
							</p>

							<span>→</span>

							"<span class="truncate" title={workflow.newMessage}>
								{workflow.newMessage}
							</span>"
						</div>
					</div>
				</div>
			{:else}
				<span class="text-13 text-greyer">Reword action without subject</span>
			{/if}
		</div>
	{:else if kind.type === 'renameBranch'}
		<div class="text-13">
			<div class="operations">
				<div class="operation-row text-13">
					<div class="operation-icon">
						<Icon name="branch-local" />
					</div>

					<div class="operation-content">
						<p class="operation__title">Renamed branch</p>
						<span>→</span>
						<button
							class="operation__commit-branch truncate"
							type="button"
							onclick={() => selectBranch(kind.subject.stackId, kind.subject.newBranchName)}
						>
							<span class="truncate" title={kind.subject.newBranchName}>
								{kind.subject.newBranchName}
							</span>
						</button>
					</div>
				</div>
			</div>
		</div>
	{/if}
{:else if rest.type === 'tool-call'}
	{@const parsedCall = parseToolCall(rest.toolCall)}

	<div class="text-13">
		<div class="operations">
			<div class="operation-row text-13">
				<div class="operation-icon" class:error={parsedCall.isError}>
					<Icon name={getToolCallIcon(parsedCall.name, parsedCall.isError)} />
				</div>

				<div class="operation-content">
					{#if parsedCall.name === 'commit'}
						{@const commitTitle = parsedCall.parameters?.messageTitle ?? ''}
						{@const commmitBody = parsedCall.parameters?.messageBody ?? ''}
						{@const commitMessage = commitTitle + (commmitBody ? `\n\n${commmitBody}` : '')}

						<p class="operation__title">
							Created commit:
							<span>
								{parsedCall.parsedResult?.result.newCommit?.substring(0, 7)}
							</span>
						</p>
						<span>→</span>

						"<span class="truncate" title={commitMessage}>
							{commitMessage}
						</span>"
					{:else if parsedCall.name === 'create_branch'}
						<p class="operation__title">
							Created branch:
							<span>
								{parsedCall.parameters?.branchName ?? '-'}
							</span>
						</p>
					{:else if parsedCall.name === 'amend'}
						{@const commitTitle = parsedCall.parameters?.messageTitle ?? ''}
						{@const commmitBody = parsedCall.parameters?.messageBody ?? ''}
						{@const commitMessage = commitTitle + (commmitBody ? `\n\n${commmitBody}` : '')}

						<p class="operation__title">
							Amended commit:
							<span>
								{parsedCall.parsedResult?.result.newCommit?.substring(0, 7)}
							</span>
						</p>
						<span>→</span>

						"<span class="truncate" title={commitMessage}>
							{commitMessage}
						</span>"
					{:else if parsedCall.name === 'create_blank_commit'}
						{@const commitTitle = parsedCall.parameters?.messageTitle ?? ''}
						{@const commmitBody = parsedCall.parameters?.messageBody ?? ''}
						{@const commitMessage = commitTitle + (commmitBody ? `\n\n${commmitBody}` : '')}

						<p class="operation__title">Created blank commit in branch</p>
						<span>→</span>

						"<span class="truncate" title={commitMessage}>
							{commitMessage}
						</span>"
					{:else if parsedCall.name === 'get_project_status'}
						<p class="operation__title">Reading the project status</p>
					{:else if parsedCall.name === 'move_file_changes'}
						<p class="operation__title">
							Moving changes from
							<span>
								{parsedCall.parameters?.sourceCommitId.substring(0, 7)}
							</span>
							<span>→</span>
							<span>
								{parsedCall.parameters?.destinationCommitId.substring(0, 7)}
							</span>
						</p>
					{:else if parsedCall.name === 'get_commit_details'}
						<p class="operation__title">
							Reading commit details for
							<span>
								{parsedCall.parameters?.commitId.substring(0, 7)}
							</span>
						</p>
					{:else if parsedCall.name === 'squash_commits'}
						<p class="operation__title">Squashed commits</p>
					{:else if parsedCall.name === 'split_branch'}
						<p class="operation__title">
							Split branch
							<span>
								{parsedCall.parameters?.sourceBranchName}
							</span>
							<span>→</span>
							<span>
								{parsedCall.parameters?.newBranchName}
							</span>
						</p>
					{:else if parsedCall.name === 'get_branch_changes'}
						<p class="operation__title">
							Reading branch changes for
							<span>
								{parsedCall.parameters?.branchName}
							</span>
						</p>
					{:else if parsedCall.name === 'split_commit'}
						<p class="operation__title">
							Split commit
							<span>
								{parsedCall.parameters?.sourceCommitId.substring(0, 7)}
							</span>
						</p>
					{/if}
				</div>

				<Button
					icon={isExpanded ? 'chevron-up-small' : 'chevron-down-small'}
					kind="ghost"
					onclick={() => {
						isExpanded = !isExpanded;
					}}
				/>
			</div>
		</div>
		<div class="tool-call-info">
			{#if isExpanded}
				{@render code(parsedCall.rawParameters)}
				{@render code(parsedCall.rawResult)}
			{/if}
		</div>
	</div>
{/if}

<style lang="postcss">
	.operations {
		display: flex;
		align-items: center;
		width: 100%;
		min-width: 0;
		overflow: hidden;
	}

	.operation-row {
		display: flex;
		align-items: center;
		width: 100%;
		min-width: 0;
		gap: 8px;
	}

	.operation-icon {
		display: flex;
		align-items: center;
		padding-top: 1px;
		color: var(--clr-text-2);

		&.error {
			color: var(--clr-core-err-50);
		}
	}

	.operation-content {
		display: flex;
		align-items: center;
		min-width: 0;
		gap: 2px;
	}

	.operation__title {
		text-wrap: nowrap;
	}

	.operation__commit-branch,
	.operation__commit-sha {
		gap: 2px;
		text-decoration: dotted underline;
		text-wrap: nowrap;
		cursor: pointer;
		transition: color var(--transition-fast);
	}

	.truncate {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.text-greyer {
		color: var(--clr-text-3);
	}

	.code-block {
		padding: 4px;
		overflow-x: scroll;
		border-radius: var(--radius-m);
		background-color: var(--clr-bg-2-muted);
	}

	.tool-call-info {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}
</style>
