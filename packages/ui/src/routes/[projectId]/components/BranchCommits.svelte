<script lang="ts">
	import type { Project } from '$lib/backend/projects';
	import ScrollableContainer from '$lib/components/ScrollableContainer.svelte';
	import type { PrService } from '$lib/github/service';
	import type { GitHubIntegrationContext } from '$lib/github/types';
	import type { BranchController } from '$lib/vbranches/branchController';
	import type { BaseBranch, Branch } from '$lib/vbranches/types';
	import CommitList from './CommitList.svelte';

	export let project: Project;
	export let branch: Branch;
	export let base: BaseBranch | undefined | null;
	export let prService: PrService;
	export let branchController: BranchController;
	export let githubContext: GitHubIntegrationContext | undefined;
	export let readonly: boolean;

	// Intended for 2 way binding.
	export let scrollable: boolean | undefined = undefined;
</script>

{#if branch.commits.length > 0}
	<ScrollableContainer bind:scrollable>
		<CommitList
			{branch}
			{base}
			{githubContext}
			{project}
			{branchController}
			{prService}
			{readonly}
			type="local"
		/>
		<CommitList
			{branch}
			{base}
			{githubContext}
			{project}
			{branchController}
			{prService}
			{readonly}
			type="remote"
		/>
		<CommitList
			{branch}
			{base}
			{githubContext}
			{project}
			{branchController}
			{prService}
			{readonly}
			type="integrated"
		/>
	</ScrollableContainer>
{/if}
