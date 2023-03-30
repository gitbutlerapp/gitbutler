<script lang="ts">
	import type { PageData } from './$types';
	import { search, type SearchResult } from '$lib';
	import { IconChevronLeft, IconChevronRight } from '$lib/components/icons';
	import { listFiles } from '$lib/sessions';
	import { asyncDerived } from '@square/svelte-store';
	import { IconRotateClockwise } from '$lib/components/icons';
	import { format, formatDistanceToNow } from 'date-fns';
	import { list as listDeltas } from '$lib/deltas';
	import { CodeViewer } from '$lib/components';
	import { page } from '$app/stores';
	import { derived } from 'svelte/store';
	import { goto } from '$app/navigation';

	export let data: PageData;
	const { project } = data;

	const limit = 10;
	const query = derived(page, (page) => page.url.searchParams.get('q'));
	const offset = derived(page, (page) => parseInt(page.url.searchParams.get('offset') ?? '0'));

	const openNextPage = () => goto(`?q=${$query}&offset=${$offset + limit}`);
	const openPrevPage = () => goto(`?q=${$query}&offset=${$offset - limit}`);

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
		const date = format(deltas[deltas.length - 1].timestampMs, 'yyyy-MM-dd');
		return {
			doc,
			deltas,
			filepath: filePath,
			highlight: highlighted,
			sessionId,
			projectId,
			date
		};
	};

	const { store: searchResults, state: searchState } = asyncDerived(
		[query, project, offset],
		async ([query, project, offset]) => {
			if (!query || !project) return { page: [], total: 0, haveNext: false, havePrev: false };
			const results = await search({ projectId: project.id, query, limit, offset });
			return {
				page: await Promise.all(results.page.map(fetchResultData)),
				haveNext: offset + limit < results.total,
				havePrev: offset > 0,
				total: results.total
			};
		},
		{ trackState: true }
	);
</script>

<figure id="search-results" class="flex h-full flex-col gap-10 px-14">
	{#if $searchState?.isLoading || $searchState?.isReloading}
		<figcaption>
			<p class="mb-2 text-xl text-[#D4D4D8]">Searching for "{$query}"...</p>
		</figcaption>

		<div class="mx-auto">
			<IconRotateClockwise class="h-20 w-20 animate-spin" />
		</div>
	{:else if $searchState?.isError}
		<figcaption>
			<p class="mb-2 text-xl text-[#D4D4D8]">Error searching for "{$query}"</p>
		</figcaption>
	{:else if $searchState?.isLoaded}
		<figcaption class="mt-14">
			{#if $searchResults.total > 0}
				<p class="mb-2 text-xl text-[#D4D4D8]">Results for "{$query}"</p>
				<p class="text-lg text-[#717179]">{$searchResults.total} change instances</p>
			{:else}
				<p class="mb-2 text-xl text-[#D4D4D8]">No results for "{$query}"</p>
			{/if}
		</figcaption>

		<ul class="search-result-list -mr-14 flex flex-auto flex-col gap-6 overflow-auto pb-6">
			{#each $searchResults.page as { doc, deltas, filepath, highlight, sessionId, projectId, date }}
				{@const timestamp = deltas[deltas.length - 1].timestampMs}
				<li class="search-result mr-14">
					<a
						href="/projects/{projectId}/player/{date}/{sessionId}?delta={deltas.length -
							1}&file={encodeURIComponent(filepath)}"
						class="flex flex-col gap-2"
					>
						<p class="flex justify-between text-lg">
							<span>{filepath}</span>
							<span>{formatDistanceToNow(timestamp)} ago</span>
						</p>
						<div
							class="flex-auto overflow-auto rounded-lg border border-zinc-700 bg-[#2F2F33] p-2 text-[#EBDBB2] shadow-lg"
						>
							<CodeViewer {doc} {deltas} {filepath} paddingLines={2} {highlight} />
						</div>
					</a>
				</li>
			{/each}

			<nav class="mx-auto flex  text-zinc-400">
				<button
					on:click={openPrevPage}
					disabled={!$searchResults.havePrev}
					title="Back"
					class:text-zinc-50={$searchResults.havePrev}
					class="h-9 w-9 rounded-tl-md rounded-bl-md border border-r-0 border-zinc-700 hover:bg-zinc-700"
				>
					<IconChevronLeft class="ml-1 h-5 w-6" />
				</button>
				<button
					on:click={openNextPage}
					disabled={!$searchResults.haveNext}
					title="Next"
					class:text-zinc-50={$searchResults.haveNext}
					class="h-9 w-9 rounded-tr-md rounded-br-md border border-l border-zinc-700 hover:bg-zinc-700"
				>
					<IconChevronRight class="ml-1 h-5 w-6" />
				</button>
			</nav>
		</ul>
	{/if}
</figure>

<style>
	/* this is trick to make webkit use hardware acceleration */
	figure * {
		-webkit-transform: translate3d(0, 0, 0);
		-webkit-perspective: 1000;
	}
</style>
