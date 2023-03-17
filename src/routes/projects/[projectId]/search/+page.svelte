<script lang="ts">
	import type { PageData } from './$types';
	import { search } from '$lib';
	import { getContext } from 'svelte';
	import type { Writable } from 'svelte/store';
	import { listFiles } from '$lib/sessions';
	import { list as listDeltas, type Delta } from '$lib/deltas';
	import ResultSnippet from './ResultSnippet.svelte';

	export let data: PageData;
	const { project } = data;

	let processedResults = [] as {
		doc: string;
		deltas: Delta[];
		filepath: string;
		highlight: string[];
	}[];
	let searchTerm: Writable<string> = getContext('searchTerm');
	let stopProcessing = false;

	$: {
		stopProcessing = true;
		if ($searchTerm) {
			fetchResults($project?.id ?? '', $searchTerm);
		}
	}

	const fetchResults = async (projectId: string, query: string) => {
		const results = await search({ projectId, query });
		stopProcessing = false;
		for (const result of results) {
			if (stopProcessing) {
				processedResults = [];
				stopProcessing = false;
				return;
			}
			const { sessionId, projectId, filePath } = result;
			const [doc, deltas] = await Promise.all([
				listFiles({ projectId, sessionId, paths: [filePath] }).then((r) => r[filePath] ?? ''),
				listDeltas({ projectId, sessionId, paths: [filePath] }).then((r) => r[filePath] ?? [])
			]);
			processedResults = [
				...processedResults,
				{
					doc,
					deltas,
					filepath: filePath,
					highlight: result.highlighted
				}
			];
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
		{:else}
			<div class="mb-10 mt-14">
				<p class="mb-2 text-xl text-[#D4D4D8]">No results for "{$searchTerm}"</p>
			</div>
		{/if}

		<ul class="flex flex-col gap-4">
			{#each processedResults as { doc, deltas, filepath, highlight }}
				<li>
					<ResultSnippet {doc} {deltas} {filepath} mark={highlight} />
				</li>
			{/each}
		</ul>
	</div>
</figure>
