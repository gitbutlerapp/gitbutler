<script lang="ts">
	import type { PageData } from './$types';
	import { search } from '$lib';
	import { onMount } from 'svelte';
	import { RenderedSearchResult, type ProcessedSearchResult } from '$lib/components/search';
	import { processSearchResult } from '$lib/components/search/process';

	export let data: PageData;
	const { project } = data;

	const urlParams = new URLSearchParams(window.location.search);
	const query = urlParams.get('search');

	onMount(async () => {
		if (!$project) return;
		if (query) fetchResults($project.id, query);
	});

	let processedResults = [] as ProcessedSearchResult[];

	const fetchResults = async (projectId: string, query: string) => {
		const result = await search({ projectId, query });
		for (const r of result) {
			const processedResult = await processSearchResult(r, query);
			processedResults = [...processedResults, processedResult];
		}
	};
</script>

<figure class="flex flex-col gap-2">
	<div class="mx-14 ">
		{#if processedResults.length > 0}
			<div class="mb-10 mt-14">
				<p class="mb-2 text-xl text-[#D4D4D8]">Results for "{query}"</p>
				<p class="text-lg text-[#717179]">{processedResults.length} change instances</p>
			</div>
		{/if}

		<ul class="flex flex-col gap-4">
			{#each processedResults as r}
				<li>
					<RenderedSearchResult processedResult={r} />
				</li>
			{/each}
		</ul>
	</div>
</figure>
