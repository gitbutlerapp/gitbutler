<script lang="ts">
	import FullviewLoading from '$lib/components/FullviewLoading.svelte';
	import RemoteBranchPreview from '$lib/components/RemoteBranchPreview.svelte';
	import { GitHubService } from '$lib/github/service';
	import { getContextByClass } from '$lib/utils/context';
	import type { PageData } from './$types';
	import { page } from '$app/stores';

	export let data: PageData;

	const githubService = getContextByClass(GitHubService);

	$: ({ error, branches } = data.remoteBranchService);

	$: branch = $branches?.find((b) => b.sha == $page.params.sha);
	$: pr = githubService.getListedPr(branch?.displayName);
</script>

{#if $error}
	<p>Error...</p>
{:else if !$branches}
	<FullviewLoading />
{:else if branch}
	<RemoteBranchPreview {pr} {branch} />
{:else}
	<p>Branch doesn't seem to exist</p>
{/if}
