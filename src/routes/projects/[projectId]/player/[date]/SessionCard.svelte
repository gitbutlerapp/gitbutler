<script lang="ts">
	import type { Delta, Session } from '$lib/api';
	import { page } from '$app/stores';
	import { collapse } from '$lib/paths';
	import { derived, type Loadable } from '@square/svelte-store';
	import { stores } from '$lib';
	import { IconBookmarkFilled } from '$lib/icons';

	export let isCurrent: boolean;
	export let session: Session;
	export let currentFilepath: string;
	export let deltas: Loadable<Record<string, Delta[]>>;

	$: bookmarks = derived(
		[stores.bookmarks({ projectId: session.projectId }), deltas],
		([bookmarks, deltas]) => {
			if (bookmarks.isLoading) return [];
			const timestamps = Object.values(deltas ?? {}).flatMap((deltas) =>
				deltas.map((d) => d.timestampMs)
			);
			const start = Math.min(...timestamps);
			const end = Math.max(...timestamps);
			return bookmarks.value
				.filter((bookmark) => !bookmark.deleted)
				.filter((bookmark) => bookmark.timestampMs >= start && bookmark.timestampMs < end);
		}
	);

	const unique = (value: any, index: number, self: any[]) => self.indexOf(value) === index;
	const lexically = (a: string, b: string) => a.localeCompare(b);

	const changedFiles = derived(deltas, (deltas) => Object.keys(deltas ?? {}).filter(unique));

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
</script>

<li
	id={isCurrent ? 'current-session' : ''}
	class:bg-card-active={isCurrent}
	class="session-card relative rounded border-[0.5px] border-gb-700 text-zinc-300 shadow-md"
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
			{$changedFiles.length}
			{$changedFiles.length !== 1 ? 'files' : 'file'}
		</span>

		{#if isCurrent}
			{#await changedFiles.load() then}
				<ul
					class="list-disk list-none overflow-hidden rounded-bl rounded-br bg-zinc-800 py-1 pl-0 pr-2"
					style:list-style="disc"
				>
					{#each $changedFiles.sort(lexically) as filename}
						<li
							class:text-zinc-100={currentFilepath === filename}
							class:bg-[#3356C2]={currentFilepath === filename}
							class="mx-5 ml-1 w-full list-none rounded p-1 text-zinc-500"
						>
							{collapse(filename)}
						</li>
					{/each}
				</ul>
			{/await}
		{/if}
	</a>
</li>
