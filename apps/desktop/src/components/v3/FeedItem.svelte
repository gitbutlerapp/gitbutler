<script lang="ts">
	import { ButlerAction, Workflow, type Outcome } from '$lib/actions/types';
	import { Snapshot } from '$lib/history/types';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { UiState } from '$lib/state/uiState.svelte';
	import { User } from '$lib/user/user';
	import { getContext } from '@gitbutler/shared/context';
	import { getContextStore } from '@gitbutler/shared/context';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import TimeAgo from '@gitbutler/ui/TimeAgo.svelte';
	import Tooltip from '@gitbutler/ui/Tooltip.svelte';
	import Markdown from '@gitbutler/ui/markdown/Markdown.svelte';
	import toasts from '@gitbutler/ui/toasts';

	type Props = {
		projectId: string;
		action: ButlerAction | Snapshot | Workflow;
	};

	const { action, projectId }: Props = $props();

	const stackService = getContext(StackService);
	const uiState = getContext(UiState);

	const allStacks = $derived(stackService.allStacks(projectId));

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

	const user = getContextStore(User);
	let failedToLoadImage = $state(false);
</script>

<div class="action-item">
	{#if action instanceof ButlerAction}
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
			</div>
			<span class="text-14 text-darkgrey">
				<Markdown content={action.externalSummary} />
			</span>
			{#if action.response && action.response.updatedBranches.length > 0}
				<div class="action-item__content__metadata">
					{@render outcome(action.response)}
				</div>
			{/if}
		</div>
	{:else if action instanceof Snapshot}
		<div class="action-item__picture">
			{#if $user?.picture && !failedToLoadImage}
				<img
					class="user-icon__image"
					src={$user.picture}
					alt=""
					referrerpolicy="no-referrer"
					onerror={() => (failedToLoadImage = true)}
				/>
			{:else}
				<Icon name="profile" />
			{/if}
		</div>
		<div class="action-item__content">
			<div class="action-item__content__header">
				<div>
					<p class="text-13 text-bold">{action.details?.operation}</p>
					<span class="text-13 text-greyer"
						><TimeAgo date={new Date(action.createdAt)} addSuffix /></span
					>
				</div>
			</div>
			<span class="text-14 text-darkgrey">
				{#if action.details?.trailers}
					{#each action.details?.trailers as trailer}
						{trailer.key}
						{trailer.value}
					{/each}
				{/if}
				{#if action.details?.body}
					<Markdown content={action.details?.body} />
				{/if}
				{#each action.filesChanged as file}
					<span class="text-greyer">{file}</span>
				{/each}
			</span>
		</div>
	{:else if action instanceof Workflow}
		<div>
			Workflow {action.id}
			{action.createdAt}
			{JSON.stringify(action.triggeredBy)}
			{JSON.stringify(action.status)}
			{JSON.stringify(action.inputCommits)}
			{JSON.stringify(action.outputCommits)}
		</div>
	{/if}
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
	.action-item__picture {
		display: flex;
		align-items: center;
		justify-content: center;

		width: 30px;
		min-width: 30px;
		height: 30px;
		padding: 2px;
		border: 1px solid var(--clr-border-2);

		border-radius: var(--radius-m);
		background-color: var(--clr-bg-2);

		> img {
			border-radius: var(--radius-s);
		}
	}
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
