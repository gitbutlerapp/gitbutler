<script lang="ts">
	import ReduxResult from './ReduxResult.svelte';
	import noChanges from '$lib/assets/illustrations/no-changes.svg?raw';
	import { WorktreeService } from '$lib/worktree/worktreeService.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';

	type Props = {
		projectId: string;
	};

	const { projectId }: Props = $props();

	const worktreeService = getContext(WorktreeService);
	const { data, status, error } = $derived(worktreeService.getChanges(projectId));
</script>

<div class="worktree-header">
	<div class="text-14 text-semibold">Uncommitted changes</div>
	<Button kind="ghost" icon="sidebar-unfold" />
</div>

<div class="worktree-body">
	<ReduxResult data={data?.changes} {status} {error}>
		{#snippet children(changes)}
			{#each changes || [] as change}
				<p>{change.path}</p>
			{/each}
		{/snippet}
		{#snippet empty()}
			<div class="text-12 text-body helper-text">
				{@html noChanges}
				<div>You're all caught up!</div>
				<div>No files need committing</div>
			</div>
		{/snippet}
	</ReduxResult>
</div>

<style lang="postcss">
	.worktree-header {
		width: 100%;
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 10px 8px 10px 14px;
	}

	.worktree-body {
		flex: 1;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
	}

	.helper-text {
		text-align: center;
		color: var(--clr-text-2);
		opacity: 0.6;
		margin-top: 10px;
	}
</style>
