<script lang="ts">
	import type { PageData } from './$types';
	import { search } from '$lib';
	import { RenderedSearchResult, type ProcessedSearchResult } from '$lib/components/search';
	import { processSearchResult } from '$lib/components/search/process';
	import { getContext } from 'svelte';
	import type { Writable } from 'svelte/store';

	export let data: PageData;
	const { project } = data;

	let processedResults = [] as ProcessedSearchResult[];
	let searchTerm: Writable<string> = getContext('searchTerm');
	let stopProcessing = false;

	$: {
		stopProcessing = true;
		if ($searchTerm) {
			fetchResults($project?.id ?? '', $searchTerm);
		}
	}

	const fetchResults = async (projectId: string, query: string) => {
		const result = await search({ projectId, query });
		stopProcessing = false;
		for (const r of result) {
			if (stopProcessing) {
				processedResults = [];
				stopProcessing = false;
				return;
			}
			const processedResult = await processSearchResult(r, query);
			if (processedResult.hunks && processedResult.hunks.length > 0) {
				processedResults = [...processedResults, processedResult];
			}
		}
	};
</script>

<figure class="flex flex-col gap-2">
	<div class="mx-14 ">
		{#if processedResults.length > 0}
			<div class="mb-10 mt-14">
				<p class="mb-2 text-xl text-[#D4D4D8]">Results for "{$searchTerm}"</p>
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
