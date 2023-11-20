<script lang="ts">
	import type { PageData } from './$types';
	import { page } from '$app/stores';
	import BranchLane from '../../components/BranchLane.svelte';

	export let data: PageData;

	$: projectId = data.projectId;
	$: user$ = data.user$;
	$: githubContext$ = data.githubContext$;
	$: cloud = data.cloud;

	$: branchController = data.branchController;
	$: vbranchService = data.vbranchService;
	$: baseBranchService = data.baseBranchService;
	$: baseBranch$ = baseBranchService.base$;

	$: branches$ = vbranchService.branches$;
	$: error$ = vbranchService.branchesError$;
	$: branch = $branches$?.find((b) => b.id == $page.params.branchId);
</script>

<div class="h-full flex-grow overflow-y-auto overscroll-none p-3">
	{#if $error$}
		<p>Error...</p>
	{:else if !$branches$}
		<p>Loading...</p>
	{:else if branch}
		<BranchLane
			{branch}
			{branchController}
			base={$baseBranch$}
			{cloud}
			{projectId}
			maximized={true}
			cloudEnabled={false}
			projectPath=""
			readonly={true}
			githubContext={$githubContext$}
			user={$user$}
		/>
	{:else}
		<p>Branch no longer exists</p>
	{/if}
</div>
