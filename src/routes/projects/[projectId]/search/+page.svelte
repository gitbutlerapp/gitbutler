<script lang="ts">
	import type { PageData } from './$types';
	import { search, type SearchResult } from '$lib';
	import { listFiles } from '$lib/sessions';
	import { list as listDeltas } from '$lib/deltas';
	import { writable } from 'svelte/store';
	import { Operation } from '$lib/deltas';
	import type { Delta } from '$lib/deltas';
	import { structuredPatch } from 'diff';
	import { formatDistanceToNow } from 'date-fns';
	import { onMount } from 'svelte';

	export let data: PageData;
	const { project } = data;

	const urlParams = new URLSearchParams(window.location.search);

	let query: string;

	onMount(async () => {
		await new Promise((r) => setTimeout(r, 100));
		query = urlParams.get('search') ?? '';
		fetchResults();
	});

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

	const getDiffHunksWithSearchTerm = (original: string, deltas: Delta[], idx: number) => {
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

	const processHunkLines = (lines: string[], newStart: number) => {
		let outLines = [];

		let lineNumber = newStart;
		for (let i = 0; i < lines.length; i++) {
			let line = lines[i];

			let content = '';
			if (!line.includes(query)) {
				content = line.slice(1);
			} else {
				let firstCharIndex = line.indexOf(query);
				let lastCharIndex = firstCharIndex + query.length - 1;
				let beforeQuery = line.slice(1, firstCharIndex);
				let querySubstring = line.slice(firstCharIndex, lastCharIndex + 1);
				let afterQuery = line.slice(lastCharIndex + 1);

				content =
					beforeQuery +
					`<span class="bg-[#AC8F2F] rounded-sm">${querySubstring}</span>` +
					afterQuery;
			}

			outLines.push({
				hidden: false,
				content: content,
				operation: line.startsWith('+') ? 'add' : line.startsWith('-') ? 'remove' : 'unmodified',
				lineNumber: !line.startsWith('-') ? lineNumber : undefined,
				hasKeyword: line.includes(query)
			});

			if (!line.startsWith('-')) {
				lineNumber++;
			}
		}

		let out = [];
		for (let i = 0; i < outLines.length; i++) {
			let prevLine = outLines[i - 1];
			let nextLine = outLines[i + 1];
			let line = outLines[i];
			if (line.hasKeyword) {
				out.push(line);
			} else if (nextLine && nextLine.hasKeyword) {
				// One line of context before the relevant line
				out.push(line);
			} else if (prevLine && prevLine.hasKeyword) {
				// One line of context after the relevant line
				out.push(line);
			} else {
				line.hidden = true;
				out.push(line);
			}
		}
		return out;
	};
</script>

<figure class="flex flex-col gap-2">
	<div class="mx-14 ">
		{#if $results.length > 0}
			<div class="mb-10 mt-14">
				<p class="text-xl text-[#D4D4D8] mb-2">Results for "{query}"</p>
				<p class="text-lg text-[#717179]">{$results.length} change instances</p>
			</div>
		{/if}

		<ul class="gap-4 flex flex-col">
			{#each $results as result}
				<li class="flex flex-col">
					{#await listFiles( { projectId: result.projectId, sessionId: result.sessionId, paths: [result.filePath] } ) then files}
						{#await listDeltas( { projectId: result.projectId, sessionId: result.sessionId } ) then deltas}
							<div class="mb-4">
								<p class="mb-2 flex text-lg text-zinc-400">
									<span>{result.filePath}</span>
									<span class="flex-grow" />
									<span
										>{formatDistanceToNow(
											new Date(deltas[result.filePath][result.index].timestampMs)
										)} ago</span
									>
								</p>
								<div class="rounded-lg bg-zinc-700 border border-[#52525B] text-[#EBDBB2]">
									{#each getDiffHunksWithSearchTerm(files[result.filePath], deltas[result.filePath], result.index) as hunk, i}
										{#if i > 0}
											<div class="border-b border-[#52525B]" />
										{/if}
										<div class="px-6 py-3 flex flex-col">
											{#each processHunkLines(hunk.lines, hunk.newStart) as line}
												{#if !line.hidden}
													<div class="flex font-mono leading-4 mb-px">
														<span class="w-6 flex-shrink text-[#928374]"
															>{line.lineNumber ? line.lineNumber : ''}</span
														>
														<pre
															class="flex-grow rounded-sm 
												{line.operation === 'add'
																? 'bg-[#14FF00]/20'
																: line.operation === 'remove'
																? 'bg-[#FF0000]/20'
																: ''}
												">{@html line.content}</pre>
													</div>
												{:else}
													<!-- <span>hidden</span> -->
												{/if}
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
	</div>
</figure>
