<script lang="ts">
	import type { PageData } from './$types';
	import { search, type SearchResult } from '$lib';
	import { writable } from 'svelte/store';

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
	}, 100);
</script>

<figure class="flex flex-col gap-2">
	<figcaption>
		<input on:input={fetchResults} type="text" name="query" bind:value={query} />
	</figcaption>

	<ul class="gap-q flex flex-col">
		{#each $results as result}
			<li>{JSON.stringify(result)}</li>
		{/each}
	</ul>
</figure>
