<script lang="ts">
	import { DiffService } from '$lib/hunks/diffService.svelte';
	import { WorktreeService } from '$lib/worktree/worktreeService.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import HunkDiff from '@gitbutler/ui/HunkDiff.svelte';

	type Props = {
		path: string;
		commitId?: string;
		projectId: string;
	};

	const { projectId, path }: Props = $props();

	const worktreeService = getContext(WorktreeService);
	const diffService = getContext(DiffService);
	const changeResult = $derived(worktreeService.getChange(projectId, path).current);
	const diffResult = $derived(
		changeResult.data ? diffService.getDiff(projectId, changeResult.data).current : undefined
	);
</script>

<div class="diff-section">
	<p class="file-name">{path}</p>
	{#await diffResult}
		loading
	{:then diff}
		{#if diff?.data?.type === 'Patch'}
			{#each diff.data.subject.hunks as hunk}
				<HunkDiff filePath={path} hunkStr={hunk.diff} />
			{/each}
		{/if}
	{/await}
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
