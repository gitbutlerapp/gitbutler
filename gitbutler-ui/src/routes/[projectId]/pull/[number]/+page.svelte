<script lang="ts">
	import FullscreenLoading from '$lib/components/FullscreenLoading.svelte';
	import PullRequestPreview from '$lib/components/PullRequestPreview.svelte';
	import { map } from 'rxjs';
	import type { PageData } from './$types';
	import { page } from '$app/stores';

	export let data: PageData;

	$: branchController = data.branchController;
	$: githubService = data.githubService;
	$: pr = githubService.prs$?.pipe(
		map((prs) => prs.find((b) => b.number.toString() == $page.params.number))
	);
</script>

<div class="wrapper overflow-y-auto overscroll-none">
	<div class="inner flex">
		{#if !$pr}
			<FullscreenLoading />
		{:else if pr}
			<PullRequestPreview {branchController} pullrequest={$pr} />
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
	}
	.inner {
		padding: var(--space-16);
	}
</style>
