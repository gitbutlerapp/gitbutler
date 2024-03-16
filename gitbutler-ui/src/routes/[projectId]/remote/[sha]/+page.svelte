<script lang="ts">
	import FullscreenLoading from '$lib/components/FullscreenLoading.svelte';
	import RemoteBranchPreview from '$lib/components/RemoteBranchPreview.svelte';
	import { GitHubService } from '$lib/github/service';
	import { getContextByClass } from '$lib/utils/context';
	import type { PageData } from './$types';
	import { page } from '$app/stores';

	export let data: PageData;

	const githubService = getContextByClass(GitHubService);

	$: project$ = data.project$;
	$: remoteBranchService = data.remoteBranchService;
	$: branches$ = remoteBranchService.branches$;
	$: error$ = remoteBranchService.branchesError$;
	$: base$ = data.baseBranchService.base$;

	$: branch = $branches$?.find((b) => b.sha == $page.params.sha);
	$: pr = githubService.getPr(branch?.displayName);
</script>

{#if $error$}
	<p>Error...</p>
{:else if !$branches$}
	<FullscreenLoading />
{:else if branch}
	<RemoteBranchPreview
		projectId={$project$.id}
		projectPath={$project$.path}
		project={$project$}
		base={$base$}
		{pr}
		{branch}
	/>
{:else}
	<p>Branch doesn't seem to exist</p>
{/if}
