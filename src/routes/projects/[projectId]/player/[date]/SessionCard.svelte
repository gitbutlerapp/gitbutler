<script lang="ts">
	import { Operation, type Delta, type Session } from '$lib/api';
	import { page } from '$app/stores';
	import { collapse } from '$lib/paths';
	import { derived } from '@square/svelte-store';
	import { stores } from '$lib';
	import { IconBookmarkFilled } from '$lib/icons';
	import { Value } from 'svelte-loadable-store';
	import { line } from '$lib/diff';
	import { Stats } from '$lib/components';

	export let isCurrent: boolean;
	export let session: Session;
	export let currentFilepath: string;
	export let deltas: Record<string, Delta[]>;
	export let files: Record<string, string>;

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

	$: stats = Object.entries(deltas)
		.map(([path, deltas]) => {
			const doc = files[path] ?? '';
			const left = deltas.length > 0 ? applyDeltas(doc, deltas.slice(0, deltas.length - 1)) : doc;
			const right = deltas.length > 0 ? applyDeltas(left, deltas.slice(deltas.length - 1)) : left;
			const diff = line(left.split('\n'), right.split('\n'));
			const linesAdded = diff
				.filter((d) => d[0] === 1)
				.map((d) => d[1].length)
				.reduce((a, b) => a + b, 0);
			const linesRemoved = diff
				.filter((d) => d[0] === -1)
				.map((d) => d[1].length)
				.reduce((a, b) => a + b, 0);
			return [linesAdded, linesRemoved];
		})
		.reduce((a, b) => [a[0] + b[0], a[1] + b[1]], [0, 0]);

	$: bookmarks = derived(stores.bookmarks.list({ projectId: session.projectId }), (bookmarks) => {
		if (bookmarks.isLoading) return [];
		if (Value.isError(bookmarks.value)) return [];
		const timestamps = Object.values(deltas ?? {}).flatMap((deltas) =>
			deltas.map((d) => d.timestampMs)
		);
		const start = Math.min(...timestamps);
		const end = Math.max(...timestamps);
		return bookmarks.value
			.filter((bookmark) => !bookmark.deleted)
			.filter((bookmark) => bookmark.timestampMs >= start && bookmark.timestampMs < end);
	});

	const unique = (value: any, index: number, self: any[]) => self.indexOf(value) === index;
	const lexically = (a: string, b: string) => a.localeCompare(b);

	$: changedFiles = Object.keys(deltas ?? {}).filter(unique);

	const sessionDuration = (session: Session) =>
		`${Math.round((session.meta.lastTimestampMs - session.meta.startTimestampMs) / 1000 / 60)} min`;

	const sessionRange = (session: Session) => {
		const day = new Date(session.meta.startTimestampMs).toLocaleString('en-US', {
			month: 'short',
			day: 'numeric'
		});
		const start = new Date(session.meta.startTimestampMs).toLocaleString('en-US', {
			hour: 'numeric',
			minute: 'numeric'
		});
		const end = new Date(session.meta.lastTimestampMs).toLocaleString('en-US', {
			hour: 'numeric',
			minute: 'numeric'
		});
		return `${day} ${start} - ${end}`;
	};

	const removeFromSearchParams = (params: URLSearchParams, key: string) => {
		params.delete(key);
		return params;
	};

	const getSessionURI = (sessionId: string) =>
		`/projects/${$page.params.projectId}/player/${
			$page.params.date
		}/${sessionId}?${removeFromSearchParams($page.url.searchParams, 'delta').toString()}`;

	let card: HTMLLIElement;
	$: if (isCurrent) {
		card?.scrollIntoView({ behavior: 'smooth' });
	}
</script>

<li
	bind:this={card}
	id={isCurrent ? 'current-session' : ''}
	class:bg-card-active={isCurrent}
	class="session-card relative rounded border-[0.5px] border-gb-700 text-zinc-300 shadow-md transition-colors duration-200 ease-in-out hover:bg-card-active"
>
	{#await bookmarks.load() then}
		{#if $bookmarks?.length > 0}
			<div class="absolute top-0 right-5 flex gap-2 overflow-hidden text-bookmark-selected">
				<IconBookmarkFilled class="-mt-1 h-4 w-4" />
			</div>
		{/if}
	{/await}

	<a href={getSessionURI(session.id)} class:pointer-events-none={isCurrent} class="w-full">
		<div class="flex flex-row justify-between rounded-t px-3 pt-3">
			<span>{sessionRange(session)}</span>
			<span>{sessionDuration(session)}</span>
		</div>

		<span class="flex flex-row justify-between px-3 pb-3">
			{changedFiles.length}
			{changedFiles.length !== 1 ? 'files' : 'file'}

			<Stats added={stats[0]} removed={stats[1]} />
		</span>

		{#if isCurrent}
			<ul
				class="list-disk list-none overflow-hidden rounded-bl rounded-br bg-zinc-800 py-1 pl-0 pr-2"
				style:list-style="disc"
			>
				{#each changedFiles.sort(lexically) as filename}
					<li
						class:text-zinc-100={currentFilepath === filename}
						class:bg-[#3356C2]={currentFilepath === filename}
						class="mx-5 ml-1 w-full list-none rounded p-1 text-zinc-500"
					>
						{collapse(filename)}
					</li>
				{/each}
			</ul>
		{/if}
	</a>
</li>
