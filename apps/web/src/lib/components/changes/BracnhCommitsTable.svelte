<script lang="ts">
	import BracnhCommitsRow from './BracnhCommitsRow.svelte';
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
	<!-- <Table headColumns={['Status', 'Name', 'Changes', 'Last Updated', 'Authors', 'Reviewers', 'Comments']}> -->
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
			{#each branch.patchIds || [] as changeId, index}
				<BracnhCommitsRow
					{changeId}
					params={data}
					branchUuid={branch.uuid}
					last={index === branch.patchIds.length - 1}
				/>
			{/each}
		{/snippet}
	</Table>
</table>

<style lang="postcss">
</style>
