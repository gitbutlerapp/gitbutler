<script lang="ts">
	import type { Project } from '$lib/backend/projects';
	import ScrollableContainer from '$lib/components/ScrollableContainer.svelte';
	import type { GitHubService } from '$lib/github/service';
	import type { BranchController } from '$lib/vbranches/branchController';
	import type { BaseBranch, Branch } from '$lib/vbranches/types';
	import CommitList from './CommitList.svelte';

	export let project: Project;
	export let branch: Branch;
	export let base: BaseBranch | undefined | null;
	export let githubService: GitHubService;
	export let branchController: BranchController;
	export let readonly: boolean;

	// Intended for 2 way binding.
	export let scrollable: boolean | undefined = undefined;
</script>

{#if branch.commits.length > 0}
	<!-- Note that 11.25rem min height is just observational, it might need updating -->
	<ScrollableContainer bind:scrollable minHeight="9rem" showBorderWhenScrolled>
		<CommitList
			{branch}
			{base}
			{project}
			{branchController}
			{githubService}
			{readonly}
			type="local"
		/>
		<CommitList
			{branch}
			{base}
			{project}
			{branchController}
			{githubService}
			{readonly}
			type="remote"
		/>
		<CommitList
			{branch}
			{base}
			{project}
			{branchController}
			{githubService}
			{readonly}
			type="integrated"
		/>
	</ScrollableContainer>
{/if}
