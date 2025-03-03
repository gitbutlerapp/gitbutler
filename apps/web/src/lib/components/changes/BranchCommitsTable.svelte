<script lang="ts">
	import BranchCommitsRow from './BranchCommitsRow.svelte';
	import Table from '$lib/components/table/Table.svelte';
	import { type Branch } from '@gitbutler/shared/branches/types';
	import { type ProjectReviewParameters } from '@gitbutler/shared/routing/webRoutes.svelte';

	type Props = {
		data: ProjectReviewParameters;
		branch: Branch;
	};

	const { data, branch }: Props = $props();
</script>

<table class="commits-table">
	<Table
		headColumns={[
			{ key: 'status', value: 'Status' },
			{ key: 'string', value: 'Name' },
			{ key: 'changes', value: 'Changes' },
			{ key: 'date', value: 'Updated' },
			{ key: 'avatars', value: 'Authors' },
			{ key: 'reviewers', value: 'Reviewers' },
			{ key: 'comments', value: '' }
		]}
	>
		{#snippet body()}
			{#each branch.patchCommitIds || [] as changeId, index}
				<BranchCommitsRow
					{changeId}
					params={data}
					branchUuid={branch.uuid}
					last={index === branch.patchCommitIds.length - 1}
				/>
			{/each}
		{/snippet}
	</Table>
</table>

<style lang="postcss">
</style>
