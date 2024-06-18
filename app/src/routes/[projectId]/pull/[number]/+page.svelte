<script lang="ts">
	// This page is displayed when:
	// - A pr is found
	// - And it does NOT have a cooresponding vbranch
	// - And it does NOT have a cooresponding remote
	// It may also display details about a cooresponding pr if they exist
	import FullviewLoading from '$lib/components/FullviewLoading.svelte';
	import PullRequestPreview from '$lib/components/PullRequestPreview.svelte';
	import { GitHubService } from '$lib/github/service';
	import { getContext } from '$lib/utils/context';
	import { map } from 'rxjs';
	import { page } from '$app/stores';

	const githubService = getContext(GitHubService);

	$: pr$ = githubService.prs$?.pipe(
		map((prs) => prs.find((b) => b.number.toString() === $page.params.number))
	);
</script>

<div class="wrapper">
	<div class="inner">
		{#if !$pr$}
			<FullviewLoading />
		{:else if pr$}
			<PullRequestPreview pullrequest={$pr$} />
		{:else}
			<p>Branch doesn't seem to exist</p>
		{/if}
	</div>
</div>

<style lang="postcss">
	.wrapper {
		display: flex;
		height: 100%;
		overflow-y: auto;
		overscroll-behavior: none;
	}
	.inner {
		display: flex;
		padding: 16px;
	}
</style>
