<script lang="ts">
	import DataContextMenu from '$components/v3/DataContextMenu.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { UiState } from '$lib/state/uiState.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import TimeAgo from '@gitbutler/ui/TimeAgo.svelte';
	import Tooltip from '@gitbutler/ui/Tooltip.svelte';
	import Markdown from '@gitbutler/ui/markdown/Markdown.svelte';
	import toasts from '@gitbutler/ui/toasts';
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

	const stackService = getContext(StackService);
	const uiState = getContext(UiState);

	const allStacks = $derived(stackService.allStacks(projectId));

	// const [revertSnapshot] = actionService.revertSnapshot;

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

	async function selectCommit(branchName: string, id: string) {
		if (!allStacks.current.data) {
			toasts.success('This commit is no longer present');
			return;
		}

		const stack = allStacks.current.data.find((stack) =>
			stack.heads.some((head) => head.name === branchName)
		);

		if (!stack) {
			toasts.success('This commit is no longer present');
			return;
		}

		const commits = await stackService.fetchCommits(projectId, stack.id, branchName);
		if (!commits.data) {
			toasts.success('This commit is no longer present');
			return;
		}

		// If the commit is not in the stack anymore, just return.
		if (!commits.data.some((commit) => commit.id === id)) {
			toasts.success('This commit is no longer present');
			return;
		}

		const stackState = uiState.stack(stack.id);
		// If it's already selected, clear the selection
		if (stackState.selection.current?.commitId === id) {
			stackState.selection.set(undefined);
		} else {
			stackState.selection.set({ branchName, commitId: id, upstream: false });
		}
	}
</script>

<DataContextMenu
	bind:open={showActions}
	items={[
		[
			{
				label: 'Revert to before',
				onclick: async () => {}
				// await restore(
				// 	action.action.subject.snapshotBefore,
				// 	`> ${action.action.subject.externalSummary}\n\nReverted to before MCP call`
				// )
			},
			{
				label: 'Revert to after',
				onclick: async () => {}
				// await restore(
				// 	action.action.subject.snapshotAfter,
				// 	`> ${action.action.subject.externalSummary}\n\nReverted to after MCP call`
				// )
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
				<Tooltip text={action.externalPrompt}><div class="pill text-12">Prompt</div></Tooltip>
			</div>
			<div bind:this={showActionsTarget}>
				<Button icon="kebab" size="tag" kind="outline" onclick={() => (showActions = true)} />
			</div>
		</div>
		<span class="text-14 text-darkgrey">
			<Markdown content={action.externalSummary} />
		</span>
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

{#snippet outcome(outcome: Outcome)}
	{#each outcome.updatedBranches as branch}
		{#if branch.newCommits.length > 0}
			{@const stack = allStacks.current.data?.find((stack) =>
				stack.heads.some((head) => head.name === branch.branchName)
			)}
			{@const stackState = stack ? uiState.stack(stack?.id) : undefined}

			{#each branch.newCommits as commit}
				{@const selected = stackState?.selection.current?.commitId === commit}
				<!-- svelte-ignore a11y_click_events_have_key_events -->
				<!-- svelte-ignore a11y_no_static_element_interactions -->
				<div
					class="outcome-item"
					onclick={() => selectCommit(branch.branchName, commit)}
					class:selected
				>
					<Icon name="commit" />
					<p class="text-14">
						Created commit {commit.slice(0, 7)} on {branch.branchName}
					</p>
				</div>
			{/each}
		{:else}
			<div class="outcome-item">
				<Icon name="branch-small" />
				<p class="text-14">Updated branch {branch.branchName}</p>
			</div>
		{/if}
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
		overflow: hidden;

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

		&.selected {
			background-color: var(--clr-core-ntrl-95);
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
