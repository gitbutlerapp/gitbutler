<script lang="ts">
	import type { PageData } from './$types';
	import { search, type SearchResult } from '$lib';
	import { listFiles } from '$lib/sessions';
	import { list as listDeltas } from '$lib/deltas';
	import { writable } from 'svelte/store';
	import { CodeViewer } from '$lib/components';
	import { Operation } from '$lib/deltas';

	export let data: PageData;
	const { project } = data;

	let query: string;

	const results = writable<SearchResult[]>([]);

	const debounce = <T extends (...args: any[]) => any>(fn: T, delay: number) => {
		let timeout: ReturnType<typeof setTimeout>;
		return (...args: any[]) => {
			clearTimeout(timeout);
			timeout = setTimeout(() => fn(...args), delay);
		};
	};

	const fetchResults = debounce(async () => {
		if (!$project) return;
		if (!query) return results.set([]);
		search({ projectId: $project.id, query }).then(results.set);
	}, 500);
</script>

<figure class="flex flex-col gap-2">
	<figcaption>
		<input on:input={fetchResults} type="text" name="query" bind:value={query} />
	</figcaption>

	<ul class="gap-q flex flex-col">
		{#each $results as result}
			<li>
				{#await listFiles( { projectId: result.projectId, sessionId: result.sessionId, paths: [result.filePath] } ) then files}
					{#await listDeltas( { projectId: result.projectId, sessionId: result.sessionId } ) then deltas}
						<div class="m-4">
							<p class="mb-2 text-lg font-bold">{result.filePath}</p>
							<div class="border border-red-400 ">
								<!-- {JSON.stringify(deltas[result.filePath][result.index])} -->
								<CodeViewer
									doc={files[result.filePath] || ''}
									filepath={result.filePath}
									deltas={[deltas[result.filePath][result.index]] || []}
									highlightLatest={true}
								/>
							</div>
						</div>
					{/await}
				{/await}
			</li>
		{/each}
	</ul>
</figure>
