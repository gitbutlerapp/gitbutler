<script lang="ts">
	import type { PageData } from './$types';
	import { page } from '$app/stores';
	import ProjectHeader from '../../ProjectHeader.svelte';
	import PullRequestPreview from './PullRequestPreview.svelte';

	export let data: PageData;
	let { branchController, pullRequestsState, pullRequestsStore } = data;

	$: pr = $pullRequestsStore?.find((b) => b.number.toString() == $page.params.number);
</script>

<div class="bg-color-3 flex h-full flex-grow flex-col overflow-y-auto overscroll-none">
	<div class="flex-grow px-8">
		{#if $pullRequestsState?.isLoading}
			<p>Loading...</p>
		{:else if $pullRequestsState?.isError}
			<p>Error...</p>
		{:else if pr}
			<PullRequestPreview {branchController} pullrequest={pr} />
		{:else}
			<p>Branch doesn't seem to exist</p>
		{/if}
	</div>
</div>
