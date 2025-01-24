<script lang="ts">
	import ChangeList from './ChangeList.svelte';
	import ReduxResult from './ReduxResult.svelte';
	import noChanges from '$lib/assets/illustrations/no-changes.svg?raw';
	import { createCommitStore } from '$lib/commits/contexts';
	import { IdSelection } from '$lib/selection/idSelection.svelte';
	import { WorktreeService } from '$lib/worktree/worktreeService.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import { setContext } from 'svelte';

	type Props = {
		projectId: string;
	};

	const { projectId }: Props = $props();

	const worktreeService = getContext(WorktreeService);
	const idSelection = new IdSelection(worktreeService);
	setContext(IdSelection, idSelection);
	createCommitStore(undefined);

	const { data, status, error } = $derived(worktreeService.getChanges(projectId));
</script>

<div class="worktree-header">
	<div class="text-14 text-semibold">Uncommitted changes</div>
	<Button kind="ghost" icon="sidebar-unfold" />
</div>

<div class="file-list">
	<ReduxResult data={data?.changes} {status} {error}>
		{#snippet children(changes)}
			<ChangeList {projectId} {changes} />
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
