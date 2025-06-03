<script lang="ts">
	import ReduxResult from '$components/ReduxResult.svelte';
	import ActionService from '$lib/actions/actionService.svelte';
	import { inject } from '@gitbutler/shared/context';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import TimeAgo from '@gitbutler/ui/TimeAgo.svelte';
	import type { ButlerAction, Outcome } from '$lib/actions/types';

	type Props = {
		projectId: string;
	};

	const { projectId }: Props = $props();

	const [actionService] = inject(ActionService);

	const actions = $derived(actionService.listActions(projectId));
</script>

<div class="action-log-wrap">
	<ReduxResult {projectId} result={actions.current}>
		{#snippet children(actions)}
			<div class="action-log">
				{#if actions.total > 0}
					<div class="action-log__header">
						<h2 class="text-16 text-semibold">Action Log</h2>
					</div>
					<div class="scrollable">
						{#each actions.actions as action (action.id)}
							{@render actionItem(action)}
						{/each}
					</div>
				{:else}
					<h2 class="text-16">No actions performed, yet!</h2>
				{/if}
			</div>
		{/snippet}
	</ReduxResult>
</div>

{#snippet actionItem(action: ButlerAction)}
	<div class="action-item">
		<div class="action-item__robot">
			<Icon name="robot" />
		</div>
		<div class="action-item__content">
			<div class="action-item__content__header">
				<div>
					<p class="text-13 text-bold">Updated workspace</p>
					<p class="text-13 text-bold text-grey">(MCP call)</p>
					<span class="text-13 text-greyer"
						><TimeAgo date={new Date(action.createdAt)} addSuffix /></span
					>
				</div>
			</div>
			<span class="text-14 action-item__content__summary">{action.externalSummary}</span>
			{#if action.response && action.response.updatedBranches.length > 0}
				<div class="action-item__content__metadata">
					{@render outcome(action.response)}
				</div>
			{/if}
		</div>
	</div>
{/snippet}

{#snippet outcome(outcome: Outcome)}
	{#each outcome.updatedBranches as branch}
		<div class="outcome-item">
			{#if branch.newCommits.length > 0}
				{#each branch.newCommits as commit}
					<Icon name="commit" />
					<p class="text-14">
						Created commit {commit.slice(0, 7)} on {branch.branchName}
					</p>
				{/each}
			{:else}
				<Icon name="branch-small" />
				<p class="text-14">Updated branch {branch.branchName}</p>
			{/if}
		</div>
	{/each}
{/snippet}

<style lang="postcss">
	.action-log-wrap {
		flex-grow: 1;

		overflow: hidden;

		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml);
		background-color: var(--clr-bg-1);
	}

	.action-log__header {
		padding: 16px;

		border-bottom: 1px solid var(--clr-border-2);
	}

	.action-item__robot {
		padding: 4px 6px;
		border: 1px solid var(--clr-border-2);

		border-radius: var(--radius-m);
		background-color: var(--clr-bg-2);
	}

	.action-log {
		display: flex;
		flex-direction: column;

		height: 100%;

		gap: 16px;
	}

	.scrollable {
		display: flex;
		flex-grow: 1;
		flex-direction: column-reverse;

		padding: 16px;

		overflow: auto;

		gap: 20px;
	}

	.action-item {
		display: flex;

		align-items: flex-start;
		gap: 14px;
	}

	.action-item__content__header {
		> div {
			display: flex;
			flex-wrap: wrap;

			align-items: center;
			gap: 8px;
		}
	}

	.action-item__content {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}

	.action-item__content__metadata {
		width: 100%;

		margin-top: 4px;

		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
	}

	.action-item__content__summary {
		white-space: pre-wrap;
	}

	.outcome-item {
		display: flex;

		padding: 12px;
		gap: 8px;

		border-bottom: 1px solid var(--clr-border-2);

		&:last-child {
			border-bottom: none;
		}
	}

	.text-grey {
		color: var(--clr-text-2);
	}

	.text-greyer {
		color: var(--clr-text-3);
	}
</style>
