<script lang="ts">
	import Branches from '$components/v3/branchesPage/Branches.svelte';
	import BranchesListGroup from '$components/v3/branchesPage/BranchesListGroup.svelte';
	import CurrentOriginCard from '$components/v3/branchesPage/CurrentOriginCard.svelte';
	import { page } from '$app/state';

	const projectId = $derived(page.params.projectId!);
</script>

<div class="branches-container">
	<div class="branches__list-sidebar">
		<div class="branches__list" style="flex: 0 0 auto;">
			<BranchesListGroup title="Current workspace target">
				<CurrentOriginCard
					originName="origin/master"
					commitsAmount={13140}
					lastCommit={{
						author: 'Esteban',
						ago: '12 min',
						branch: 'gitbutler/e-branch-36',
						sha: '#ew23dk'
					}}
				/>
			</BranchesListGroup>
		</div>
		<div class="branches__list">
			<Branches {projectId} />
		</div>
	</div>
</div>

<style lang="postcss">
	.branches-container {
		display: flex;
		height: 100%;
		width: 100%;
	}

	.branches__list-sidebar {
		display: flex;
		flex-direction: column;
		gap: 8px;
		width: 300px;
		height: 100%;
		overflow: hidden;
	}

	.branches__list {
		display: flex;
		flex-direction: column;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml);
		overflow: hidden;
	}
</style>
