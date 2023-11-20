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

<div class="h-full flex-grow overflow-y-auto overscroll-none p-3">
	<div
		class="flex max-w-4xl flex-col gap-y-6 overflow-visible rounded-lg px-5 py-4"
		style:background-color="var(--bg-surface)"
		style:border-color="var(--border-surface)"
	>
		{#if !$pr}
			<p>Loading...</p>
		{:else if pr}
			<PullRequestPreview {branchController} pullrequest={$pr} />
		{:else}
			<p>Branch doesn't seem to exist</p>
		{/if}
	</div>
</div>
