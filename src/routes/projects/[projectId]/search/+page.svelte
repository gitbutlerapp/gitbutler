<script lang="ts">
	import type { PageData } from './$types';
	import { IconChevronLeft, IconChevronRight, IconLoading } from '$lib/icons';
	import { files, deltas, searchResults, type SearchResult } from '$lib/api';
	import { asyncDerived } from '@square/svelte-store';
	import { format, formatDistanceToNow } from 'date-fns';
	import { DeltasViewer } from '$lib/components';
	import { page } from '$app/stores';
	import { derived } from '@square/svelte-store';
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
		const [doc, dd] = await Promise.all([
			files.list({ projectId, sessionId, paths: [filePath] }).then((r) => r[filePath] ?? ''),
			deltas
				.list({ projectId, sessionId, paths: [filePath] })
				.then((r) => r[filePath] ?? [])
				.then((d) => d.slice(0, index + 1))
		]);
		const date = format(dd[dd.length - 1].timestampMs, 'yyyy-MM-dd');
		return {
			doc,
			deltas: dd,
			filepath: filePath,
			highlight: highlighted,
			sessionId,
			projectId,
			date
		};
	};

	const { store: results, state: searchState } = asyncDerived(
		[query, project, offset],
		async ([query, project, offset]) => {
			if (!query || !project) return { page: [], total: 0, haveNext: false, havePrev: false };
			const results = await searchResults.list({ projectId: project.id, query, limit, offset });
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
			<p class="mb-2 text-2xl text-[#D4D4D8]">Searching for "{$query}"...</p>
		</figcaption>

		<div class="mx-auto">
			<IconLoading class="h-20 w-20 animate-spin" />
		</div>
	{:else if $searchState?.isError}
		<figcaption>
			<p class="mb-2 text-2xl text-[#D4D4D8]">Error searching for "{$query}"</p>
		</figcaption>
	{:else if $searchState?.isLoaded}
		<figcaption class="mt-14">
			{#if $results.total > 0}
				<p class="mb-2 text-2xl text-[#D4D4D8]">Results for "{$query}"</p>
				<p class="text-lg text-[#717179]">{$results.total} change instances</p>
			{:else}
				<p class="mb-2 text-2xl text-[#D4D4D8]">No results for "{$query}"</p>
			{/if}
		</figcaption>

		<ul class="search-result-list -mr-14 flex flex-auto flex-col gap-6 overflow-auto pb-6">
			{#each $results.page as { doc, deltas, filepath, highlight, sessionId, projectId, date }}
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
							<DeltasViewer
								{doc}
								{deltas}
								{filepath}
								paddingLines={2}
								highlight={$query ? [$query.trim()] : []}
							/>
						</div>
					</a>
				</li>
			{/each}

			<nav class="mx-auto flex  text-zinc-400">
				<button
					on:click={openPrevPage}
					disabled={!$results.havePrev}
					title="Back"
					class:text-zinc-50={$results.havePrev}
					class="h-9 w-9 rounded-tl-md rounded-bl-md border border-r-0 border-zinc-700 hover:bg-zinc-700"
				>
					<IconChevronLeft class="ml-1 h-5 w-6" />
				</button>
				<button
					on:click={openNextPage}
					disabled={!$results.haveNext}
					title="Next"
					class:text-zinc-50={$results.haveNext}
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
		transform: translate3d(0, 0, 0);
		-webkit-perspective: 1000;
		perspective: 1000;
	}
</style>
