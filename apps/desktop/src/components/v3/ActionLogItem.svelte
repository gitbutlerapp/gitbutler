<script lang="ts">
	import DataContextMenu from '$components/v3/DataContextMenu.svelte';
	import { HistoryService } from '$lib/history/history';
	import { getContext } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import TimeAgo from '@gitbutler/ui/TimeAgo.svelte';
	import type { ButlerAction, Outcome } from '$lib/actions/types';

	type Props = {
		projectId: string;
		action: ButlerAction;
		last: boolean;
		previous: ButlerAction | undefined;
		loadNextPage: () => void;
	};

	const { action, last, previous, projectId, loadNextPage }: Props = $props();

	const historyService = getContext(HistoryService);

	async function restore(id: string) {
		await historyService.restoreSnapshot(projectId, id);
		// In some cases, restoring the snapshot doesnt update the UI correctly
		// Until we have that figured out, we need to reload the page.
		location.reload();
	}

	let lastIntersector = $state<HTMLElement>();

	$effect(() => {
		if (!lastIntersector) return;
		const observer = new IntersectionObserver((data) => {
			if (data.at(0)?.isIntersecting) {
				loadNextPage();
			}
		});
		observer.observe(lastIntersector);
		return () => observer.disconnect();
	});

	let showPreviousActions = $state(false);
	let showActions = $state(false);
	let showPreviousActionsTarget = $state<HTMLElement>();
	let showActionsTarget = $state<HTMLElement>();
</script>

<DataContextMenu
	bind:open={showActions}
	items={[
		[
			{
				label: 'Jump to before',
				onclick: async () => await restore(action.snapshotBefore)
			},
			{
				label: 'Jump to after',
				onclick: async () => await restore(action.snapshotAfter)
			}
		]
	]}
	target={showActionsTarget}
/>

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
			<div bind:this={showActionsTarget}>
				<Button icon="kebab" size="tag" kind="outline" onclick={() => (showActions = true)} />
			</div>
		</div>
		<span class="text-14 action-item__content__summary">{action.externalSummary}</span>
		{#if action.response && action.response.updatedBranches.length > 0}
			<div class="action-item__content__metadata">
				{@render outcome(action.response)}
			</div>
		{/if}
		{#if last}
			<div bind:this={lastIntersector}></div>
		{/if}
	</div>
</div>

{#if previous}
	<DataContextMenu
		bind:open={showPreviousActions}
		items={[
			[
				{
					label: 'Jump to before',
					onclick: async () => await restore(previous.snapshotAfter)
				},
				{
					label: 'Jump to after',
					onclick: async () => await restore(action.snapshotBefore)
				}
			]
		]}
		target={showPreviousActionsTarget}
	/>

	<div class="action-item">
		<div class="action-item__robot">
			<Icon name="robot" />
		</div>
		<div class="action-item__content">
			<div class="action-item__content__header">
				<div>
					<p class="text-13 text-bold">Files changed</p>
					<span class="text-13 text-greyer"
						><TimeAgo date={new Date(previous.createdAt)} addSuffix /></span
					>
				</div>
				<div bind:this={showPreviousActionsTarget}>
					<Button
						icon="kebab"
						size="tag"
						kind="outline"
						onclick={() => (showPreviousActions = true)}
					/>
				</div>
			</div>
		</div>
	</div>
{/if}

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
	.action-item__robot {
		padding: 4px 6px;
		border: 1px solid var(--clr-border-2);

		border-radius: var(--radius-m);
		background-color: var(--clr-bg-2);
	}

	.action-item {
		display: flex;

		align-items: flex-start;

		gap: 14px;
	}

	.action-item__content__header {
		display: flex;
		align-items: flex-start;

		> div:first-of-type {
			flex-grow: 1;
		}

		> div {
			display: flex;
			flex-wrap: wrap;

			align-items: center;
			gap: 8px;
		}
	}

	.action-item__content {
		display: flex;

		flex-grow: 1;
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
