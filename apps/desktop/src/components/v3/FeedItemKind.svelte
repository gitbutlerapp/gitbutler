<script lang="ts">
	import { UiState } from '$lib/state/uiState.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import type { WorkflowKind } from '$lib/actions/types';

	type Props = {
		projectId: string;
		kind: WorkflowKind;
	};

	const { kind, projectId }: Props = $props();
	const uiState = getContext(UiState);

	function selectCommit(stackId: string, branchName: string, commitId: string) {
		const projectState = uiState.project(projectId);
		const stackState = uiState.stack(stackId);
		stackState.selection.set({
			branchName,
			commitId
		});
		projectState.stackId.set(stackId);
	}
</script>

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
{/if}

<style lang="postcss">
	.operations {
		display: flex;
		align-items: center;
		width: 100%;
		min-width: 0;
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
	}

	.operation-content {
		display: flex;
		flex-grow: 1;
		align-items: center;
		min-width: 0;
		gap: 2px;
	}

	.operation__title {
		flex-grow: 1;
		width: 100%;
		text-wrap: nowrap;
	}

	.operation__commit-sha {
		display: inline-flex;
		align-items: center;
		gap: 2px;
		text-decoration: dotted underline;
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
</style>
