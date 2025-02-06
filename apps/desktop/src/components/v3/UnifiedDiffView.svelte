<script lang="ts">
	import ReduxResult from '../ReduxResult.svelte';
	import { DiffService } from '$lib/hunks/diffService.svelte';
	import { WorktreeService } from '$lib/worktree/worktreeService.svelte';
	import { inject } from '@gitbutler/shared/context';
	import HunkDiff from '@gitbutler/ui/HunkDiff.svelte';

	type Props = {
		path: string;
		commitId?: string;
		projectId: string;
	};

	const { projectId, path }: Props = $props();
	const [worktreeService, diffService] = inject(WorktreeService, DiffService);

	const result = $derived(
		worktreeService
			.getChange(projectId, path)
			.current.andThen((change) => diffService.getDiff(projectId, change)).current
	);
</script>

<div class="diff-section">
	<p class="file-name">{path}</p>
	<ReduxResult {result}>
		{#snippet children(diff)}
			{#if diff.type === 'Patch'}
				{#each diff.subject.hunks as hunk}
					<HunkDiff filePath={path} hunkStr={hunk.diff} />
				{/each}
			{/if}
		{/snippet}
	</ReduxResult>
</div>

<style lang="postcss">
	.diff-section {
		display: flex;
		padding: 14px;
		flex-direction: column;
		align-items: flex-start;
		gap: 14px;
		align-self: stretch;
		overflow-x: hidden;
		max-width: 100%;
	}

	.file-name {
		color: var(--text-1, #1a1614);

		/* base-body/12 */
		font-family: var(--fontfamily-default, Inter);
		font-size: 12px;
		font-style: normal;
		font-weight: var(--weight-regular, 400);
		line-height: 160%; /* 19.2px */
	}
</style>
