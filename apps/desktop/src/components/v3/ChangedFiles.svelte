<script lang="ts">
	import ReduxResult from '$components/ReduxResult.svelte';
	import FileList from '$components/v3/FileList.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { inject } from '@gitbutler/shared/context';
	import Badge from '@gitbutler/ui/Badge.svelte';

	interface BaseProps {
		type: 'commit' | 'branch';
		projectId: string;
	}

	interface CommitProps extends BaseProps {
		type: 'commit';
		commitId: string;
	}

	interface BranchProps extends BaseProps {
		type: 'branch';
		stackId: string;
		branchName: string;
	}

	type Props = CommitProps | BranchProps;

	const props: Props = $props();
	const [stackService] = inject(StackService);
	const commitChangesResult = $derived(
		props.type === 'commit'
			? stackService.commitChanges(props.projectId, props.commitId)
			: undefined
	);
	const branchChangesResult = $derived(
		props.type === 'branch'
			? stackService.branchChanges(props.projectId, props.stackId, props.branchName)
			: undefined
	);

	const changesResult = $derived(commitChangesResult?.current ?? branchChangesResult?.current);
</script>

{#if changesResult}
	<div class="changed-files">
		<ReduxResult result={changesResult}>
			{#snippet children(changes)}
				<div class="header text-13 text-bold">
					<span>Changed files</span>
					<Badge>{changes.length}</Badge>
				</div>
				{#if changes.length > 0}
					<FileList {changes} {...props} />
				{:else}
					<div class="text-12 text-body helper-text">(no changed files)</div>
				{/if}
			{/snippet}
		</ReduxResult>
	</div>
{:else}
	<p class="text-13 text-bold">Malformed props</p>
{/if}

<style>
	.changed-files {
		display: flex;
		flex-direction: column;
		border-radius: var(--radius-l);
		border: 1px solid var(--clr-border-2);
		overflow: hidden;
	}

	.header {
		padding: 14px 14px 16px 14px;
		display: flex;
		align-items: center;
		gap: 4px;
	}
</style>
