<script lang="ts">
	import DataContextMenu from '$components/v3/DataContextMenu.svelte';
	import { HistoryService } from '$lib/history/history';
	import { getContext } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import TimeAgo from '@gitbutler/ui/TimeAgo.svelte';
	import Tooltip from '@gitbutler/ui/Tooltip.svelte';
	import Markdown from '@gitbutler/ui/markdown/Markdown.svelte';
	import type { ButlerAction, Outcome } from '$lib/actions/types';

	type Props = {
		projectId: string;
		action: ButlerAction & { action: { type: 'mcpAction' } };
		last: boolean;
		loadNextPage: () => void;
	};

	const { action, last, projectId, loadNextPage }: Props = $props();

	// An ActionLogItem (for now) is representing both the git changes that
	// happened but also the file changes that happened between this action and
	// the previous one.
	//
	// Diffing `previous.snapshotAfter` and `action.snapshotBefore` gives us the
	// changes that happend on disk between these two events.

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
	let showActions = $state(false);
	let showActionsTarget = $state<HTMLElement>();
</script>

<DataContextMenu
	bind:open={showActions}
	items={[
		[
			{
				label: 'Revert to before',
				onclick: async () => await restore(action.action.subject.snapshotBefore)
			},
			{
				label: 'Revert to after',
				onclick: async () => await restore(action.action.subject.snapshotAfter)
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
				<Tooltip text={action.action.subject.externalPrompt}
					><div class="pill text-12">Prompt</div></Tooltip
				>
			</div>
			<div bind:this={showActionsTarget}>
				<Button icon="kebab" size="tag" kind="outline" onclick={() => (showActions = true)} />
			</div>
		</div>
		<span class="text-14 text-darkgrey">
			<Markdown content={action.action.subject.externalSummary} />
		</span>
		{#if action.action.subject.response && action.action.subject.response.updatedBranches.length > 0}
			<div class="action-item__content__metadata">
				{@render outcome(action.action.subject.response)}
			</div>
		{/if}
		{#if last}
			<div bind:this={lastIntersector}></div>
		{/if}
	</div>
</div>

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

	.text-darkgrey {
		color: var(--clr-core-ntrl-20);
		text-decoration-color: var(--clr-core-ntrl-20);
	}

	.text-greyer {
		color: var(--clr-text-3);
	}

	.pill {
		padding: 2px 6px;
		border: 1px solid var(--clr-border-2);
		border-radius: 99px;
		background-color: var(--clr-bg-2);
	}
</style>
