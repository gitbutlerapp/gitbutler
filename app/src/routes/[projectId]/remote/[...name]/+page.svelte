<script lang="ts">
	// This page is displayed when:
	// - A remote branch is found
	// - And it does NOT have a cooresponding vbranch
	// It may also display details about a cooresponding pr if they exist
	import FullviewLoading from '$lib/components/FullviewLoading.svelte';
	import RemoteBranchPreview from '$lib/components/RemoteBranchPreview.svelte';
	import { GitHubService } from '$lib/github/service';
	import { getContext } from '$lib/utils/context';
	import type { PageData } from './$types';
	import { page } from '$app/stores';

	export let data: PageData;

	const githubService = getContext(GitHubService);

	$: ({ error, branches } = data.remoteBranchService);

	$: branch = $branches?.find((b) => b.displayName === $page.params.name);
	$: pr = branch && githubService.getListedPr(branch.sha);
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
