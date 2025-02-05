<script lang="ts">
	import ChangeList from './ChangeList.svelte';
	import ReduxResult from './ReduxResult.svelte';
	import noChanges from '$lib/assets/illustrations/no-changes.svg?raw';
	import { createCommitStore } from '$lib/commits/contexts';
	import { WorktreeService } from '$lib/worktree/worktreeService.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';

	type Props = {
		projectId: string;
	};

	const { projectId }: Props = $props();

	const worktreeService = getContext(WorktreeService);
	createCommitStore(undefined);

	const result = $derived(worktreeService.getChanges(projectId));
</script>

<div class="worktree-header">
	<div class="text-14 text-semibold">Uncommitted changes</div>
	<Button kind="ghost" icon="sidebar-unfold" />
</div>

<div class="file-list">
	<ReduxResult result={result.current}>
		{#snippet children(result)}
			{#if result.changes.length > 0}
				<ChangeList {projectId} changes={result.changes} />
			{:else}
				<div class="text-12 text-body helper-text">
					{@html noChanges}
					<div>You're all caught up!</div>
					<div>No files need committing</div>
				</div>
			{/if}
		{/snippet}
	</ReduxResult>
</div>

<style>
	.worktree-header {
		width: 100%;
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 10px 8px 10px 14px;
		text-wrap: nowrap;
		overflow: hidden;

		& > div {
			width: 100%;
			overflow: hidden;
			white-space: nowrap;
			text-overflow: ellipsis;
		}
	}

	.file-list {
		display: flex;
		flex: 1;
		width: 100%;
		display: flex;
		justify-items: top;
		flex-direction: column;
		align-items: top;
		justify-content: top;
		overflow: hidden;
	}

	.helper-text {
		text-align: center;
		color: var(--clr-text-2);
		opacity: 0.6;
		margin-top: 10px;
	}
</style>
