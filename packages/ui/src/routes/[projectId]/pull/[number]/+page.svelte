<script lang="ts">
	import type { PageData } from './$types';
	import { page } from '$app/stores';
	import PullRequestPreview from './PullRequestPreview.svelte';

	export let data: PageData;
	let { branchController, pullRequestsStore } = data;

	$: pr = $pullRequestsStore?.find((b) => b.number.toString() == $page.params.number);
</script>

<div class="bg-color-3 flex h-full flex-grow flex-col overflow-y-auto overscroll-none">
	<div class="flex-grow px-8">
		{#if !$pullRequestsStore}
			<p>Loading...</p>
		{:else if pr}
			<PullRequestPreview {branchController} pullrequest={pr} />
		{:else}
			<p>Branch doesn't seem to exist</p>
		{/if}
	</div>
</div>
