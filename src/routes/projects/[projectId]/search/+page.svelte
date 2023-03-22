<script lang="ts">
	import type { PageData } from './$types';
	import { search } from '$lib';
	import { getContext } from 'svelte';
	import type { Writable } from 'svelte/store';
	import { listFiles } from '$lib/sessions';
	import { formatDistanceToNow } from 'date-fns';
	import { list as listDeltas, type Delta } from '$lib/deltas';
	import { CodeViewer } from '$lib/components';

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
		processedResults = [];
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
				listDeltas({ projectId, sessionId, paths: [filePath] })
					.then((r) => r[filePath] ?? [])
					.then((d) => d.slice(0, result.index + 1))
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

<figure class="mx-14 flex h-full flex-col gap-2">
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

	<ul class="flex-auto overflow-auto">
		{#each processedResults as { doc, deltas, filepath, highlight }}
			{@const timestamp = deltas[deltas.length - 1].timestampMs}
			<li class="mt-6">
				<div class="flex flex-col gap-2">
					<p class="flex justify-between text-lg">
						<span>{filepath}</span>
						<span>{formatDistanceToNow(timestamp)} ago</span>
					</p>
					<div
						class="flex-auto overflow-auto rounded-lg border border-zinc-700 bg-[#2F2F33] text-[#EBDBB2] drop-shadow-lg"
					>
						<CodeViewer {doc} {deltas} {filepath} paddingLines={4} {highlight} />
					</div>
				</div>
			</li>
		{/each}
	</ul>
</figure>
