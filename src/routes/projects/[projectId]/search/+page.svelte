<script lang="ts">
	import type { PageData } from './$types';
	import { search, type SearchResult } from '$lib';
	import { listFiles } from '$lib/sessions';
	import { list as listDeltas } from '$lib/deltas';
	import { writable } from 'svelte/store';
	import { Operation } from '$lib/deltas';
	import type { Delta } from '$lib/deltas';
	import { structuredPatch } from 'diff';

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
	}, 1000);

	const applyDeltas = (text: string, deltas: Delta[]) => {
		const operations = deltas.flatMap((delta) => delta.operations);

		operations.forEach((operation) => {
			if (Operation.isInsert(operation)) {
				text =
					text.slice(0, operation.insert[0]) +
					operation.insert[1] +
					text.slice(operation.insert[0]);
			} else if (Operation.isDelete(operation)) {
				text =
					text.slice(0, operation.delete[0]) +
					text.slice(operation.delete[0] + operation.delete[1]);
			}
		});
		return text;
	};

	const diffParagraph = (original: string, deltas: Delta[], idx: number) => {
		if (!original) return [];
		return structuredPatch(
			'file',
			'file',
			applyDeltas(original, deltas.slice(0, idx)),
			applyDeltas(original, [deltas[idx]]),
			'header',
			'header',
			{ context: 1 }
		).hunks.filter((hunk) => hunk.lines.some((l) => l.includes(query)));
	};
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
							<p class="mb-2 text-lg font-bold">
								{result.filePath}
								<span>{new Date(deltas[result.filePath][result.index].timestampMs)}</span>
							</p>
							<div class="border border-red-400 ">
								{#each diffParagraph(files[result.filePath], deltas[result.filePath], result.index) as hunk}
									<div class="m-4 flex flex-col border">
										{#each hunk.lines as line}
											<pre
												class={line.startsWith('+')
													? 'bg-[#14FF00]/30'
													: line.startsWith('-')
													? 'bg-[#FF0000]/30'
													: ''}>{@html line
													.slice(1)
													.split(query)
													.join(`<span class="bg-purple-400">${query}</span>`) || ' '}</pre>
										{/each}
									</div>
								{/each}
							</div>
						</div>
					{/await}
				{/await}
			</li>
		{/each}
	</ul>
</figure>
