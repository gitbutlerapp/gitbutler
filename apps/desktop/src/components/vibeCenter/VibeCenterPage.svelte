<script lang="ts">
	import ReduxResult from '$components/ReduxResult.svelte';
	import VibeCenterSidebar from '$components/vibeCenter/VibeCenterSidebar.svelte';
	import VibeCenterSidebarEntry from '$components/vibeCenter/VibeCenterSidebarEntry.svelte';
	import { invoke } from '$lib/backend/ipc';
	import { STACK_SERVICE } from '$lib/stacks/stackService.svelte';
	import { combineResults } from '$lib/state/helpers';
	import { CLAUDE_CODE_SERVICE } from '$lib/vibeCenter/claude';
	import { inject } from '@gitbutler/shared/context';
	import { Button, SidebarEntry, Textarea } from '@gitbutler/ui';

	type Props = {
		projectId: string;
	};
	const { projectId }: Props = $props();

	const claudeCodeService = inject(CLAUDE_CODE_SERVICE);
	const stackService = inject(STACK_SERVICE);

	const stackId = '7edb3b2e-869c-485b-af70-76a934e0fcfd';

	const stacks = $derived(stackService.stacks(projectId));

	let message = $state('');

	const events = $derived(claudeCodeService.transcript({ projectId, stackId }));
	$inspect(events.current.data);

	async function sendMessage() {
		await invoke('claude_send_message', {
			projectId,
			stackId,
			message
		});
		message = '';
	}
</script>

<div class="page">
	<VibeCenterSidebar content={sidebarContent}>
		{#snippet actions()}
			<Button disabled kind="outline" icon="plus-small" size="tag">Create new</Button>
		{/snippet}
	</VibeCenterSidebar>

	<div class="content"></div>
</div>

{#snippet sidebarContent()}
	<ReduxResult result={stacks.current} {projectId}>
		{#snippet children(stacks, { projectId })}
			{#each stacks as stack}
				{#each stack.heads as head}
					{@render sidebarContentEntry(projectId, stack.id, head.name)}
				{/each}
			{/each}
		{/snippet}
	</ReduxResult>
{/snippet}

{#snippet sidebarContentEntry(projectId: string, stackId: string, head: string)}
	{@const branch = stackService.branchByName(projectId, stackId, head)}
	{@const commits = stackService.commits(projectId, stackId, head)}
	<ReduxResult result={combineResults(branch.current, commits.current)} {projectId} {stackId}>
		{#snippet children([branch, commits], { projectId, stackId })}
			<VibeCenterSidebarEntry
				branchName={branch.name}
				status="vibes"
				tokensUsed={69}
				cost={4.2}
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
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-l);
		background-color: var(--clr-bg-1);
	}
</style>
