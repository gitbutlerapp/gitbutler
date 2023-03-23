<script lang="ts">
	import type { PageData } from './$types';
	import { search, type SearchResult } from '$lib';
	import { listFiles } from '$lib/sessions';
	import { asyncDerived } from '@square/svelte-store';
	import { formatDistanceToNow } from 'date-fns';
	import { list as listDeltas } from '$lib/deltas';
	import { CodeViewer } from '$lib/components';
	import { page } from '$app/stores';
	import { derived } from 'svelte/store';

	export let data: PageData;
	const { project } = data;

	const query = derived(page, (page) => page.url.searchParams.get('q'));

	const fetchResultData = async ({
		sessionId,
		projectId,
		filePath,
		index,
		highlighted
	}: SearchResult) => {
		const [doc, deltas] = await Promise.all([
			listFiles({ projectId, sessionId, paths: [filePath] }).then((r) => r[filePath] ?? ''),
			listDeltas({ projectId, sessionId, paths: [filePath] })
				.then((r) => r[filePath] ?? [])
				.then((d) => d.slice(0, index + 1))
		]);
		return {
			doc,
			deltas,
			filepath: filePath,
			highlight: highlighted
		};
	};

	const { store: searchResults, state: searchState } = asyncDerived(
		[query, project],
		async ([query, project]) => {
			if (!query || !project) return { page: [], total: 0 };
			const results = await search({ projectId: project.id, query, limit: 50 });
			return {
				page: await Promise.all(results.page.map(fetchResultData)),
				total: results.total
			};
		},
		{ trackState: true }
	);
</script>

<figure class="mx-14 flex h-full flex-col gap-2">
	{#if $searchState?.isLoading || $searchState?.isReloading}
		<figcaption class="m-auto">
			<p class="mb-2 text-xl text-[#D4D4D8]">Looking for "{$query}"...</p>
		</figcaption>
	{:else if $searchState?.isError}
		<figcaption class="m-auto">
			<p class="mb-2 text-xl text-[#D4D4D8]">Error searching for "{$query}"</p>
		</figcaption>
	{:else if $searchState?.isLoaded}
		<figcaption class="mb-10 mt-14">
			{#if $searchResults.total > 0}
				<p class="mb-2 text-xl text-[#D4D4D8]">Results for "{$query}"</p>
				<p class="text-lg text-[#717179]">{$searchResults.total} change instances</p>
			{:else}
				<p class="mb-2 text-xl text-[#D4D4D8]">No results for "{$query}"</p>
			{/if}
		</figcaption>

		<ul class="flex-auto overflow-auto">
			{#each $searchResults.page as { doc, deltas, filepath, highlight }}
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
							<CodeViewer {doc} {deltas} {filepath} paddingLines={2} {highlight} />
						</div>
					</div>
				</li>
			{/each}
		</ul>
	{/if}
</figure>
