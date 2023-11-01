<script lang="ts">
	import type { Session } from '$lib/api/ipc/sessions';
	import { isInsert, type Delta, isDelete } from '$lib/api/ipc/deltas';
	import { page } from '$app/stores';
	import { collapse } from '$lib/paths';
	import { asyncDerived } from '@square/svelte-store';
	import { getBookmarksStore } from '$lib/stores/bookmarks';
	import { IconBookmarkFilled } from '$lib/icons';
	import { line } from '$lib/diff';
	import Stats from '$lib/components/Stats.svelte';

	export let isCurrent: boolean;
	export let session: Session;
	export let currentFilepath: string;
	export let deltas: Partial<Record<string, Delta[]>>;
	export let files: Partial<Record<string, string>>;

	const applyDeltas = (text: string, deltas: Delta[]) => {
		const operations = deltas.flatMap((delta) => delta.operations);

		operations.forEach((operation) => {
			if (isInsert(operation)) {
				text =
					text.slice(0, operation.insert[0]) +
					operation.insert[1] +
					text.slice(operation.insert[0]);
			} else if (isDelete(operation)) {
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
			const left =
				deltas && deltas.length > 0 ? applyDeltas(doc, deltas.slice(0, deltas.length - 1)) : doc;
			const right =
				deltas && deltas.length > 0 ? applyDeltas(left, deltas.slice(deltas.length - 1)) : left;
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

	$: bookmarksStore = asyncDerived(
		getBookmarksStore({ projectId: session.projectId }),
		async (bookmarks) => {
			const timestamps = Object.values(deltas ?? {}).flatMap((deltas) =>
				(deltas || []).map((d) => d.timestampMs)
			);
			const start = Math.min(...timestamps);
			const end = Math.max(...timestamps);
			return bookmarks
				.filter((bookmark) => !bookmark.deleted)
				.filter((bookmark) => bookmark.timestampMs >= start && bookmark.timestampMs < end);
		}
	);

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
		`/old/${$page.params.projectId}/player/${
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
	class:bg-color-4={isCurrent}
	class="session-card border-color-2 text-color-2 hover:bg-color-4 relative rounded border-[0.5px] shadow-md transition-colors duration-200 ease-in-out"
>
	{#await bookmarksStore.load() then}
		{#if $bookmarksStore?.length > 0}
			<div class="absolute right-5 top-0 flex gap-2 overflow-hidden text-bookmark-selected">
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
				class="list-disk bg-color-2 list-none overflow-hidden rounded-bl rounded-br py-1 pl-0 pr-2"
			>
				{#each changedFiles.sort(lexically) as filename}
					<li
						class:text-zinc-100={currentFilepath === filename}
						class:bg-[#3356C2]={currentFilepath === filename}
						class="text-color-3 mx-5 ml-1 w-full list-none rounded p-1"
					>
						{collapse(filename)}
					</li>
				{/each}
			</ul>
		{/if}
	</a>
</li>
