<script lang="ts">
	import type { PageData } from './$types';
	import { page } from '$app/stores';
	import PullRequestPreview from './PullRequestPreview.svelte';
	import { map } from 'rxjs';

	export let data: PageData;

	$: branchController = data.branchController;
	$: prService = data.prService;
	$: pr = prService.prs$?.pipe(
		map((prs) => prs.find((b) => b.number.toString() == $page.params.number))
	);
</script>

<div class="wrapper overflow-y-auto overscroll-none">
	<div class="inner flex">
		{#if !$pr}
			<p>Loading...</p>
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
