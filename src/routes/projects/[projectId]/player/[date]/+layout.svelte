<script lang="ts">
	import type { LayoutData } from './$types';
	import { IconChevronLeft, IconChevronRight } from '$lib/icons';
	import { page } from '$app/stores';
	import { asyncDerived, derived } from '@square/svelte-store';
	import { format } from 'date-fns';
	import { onMount } from 'svelte';
	import { api, events, hotkeys, toasts } from '$lib';
	import BookmarkModal from './BookmarkModal.svelte';
	import tinykeys from 'tinykeys';
	import { goto } from '$app/navigation';
	import { unsubscribe } from '$lib/utils';
	import SessionCard from './SessionCard.svelte';

	export let data: LayoutData;
	const { currentFilepath, currentTimestamp } = data;

	const dateSessions = derived([data.sessions, page], ([sessions, page]) =>
		sessions?.filter(
			(session) => format(session.meta.startTimestampMs, 'yyyy-MM-dd') === page.params.date
		)
	);

	const fileFilter = derived(page, (page) => page.url.searchParams.get('file'));
	const projectId = derived(page, (page) => page.params.projectId);

	const richSessions = asyncDerived(
		[dateSessions, fileFilter, projectId],
		async ([sessions, fileFilter, projectId]) => {
			return sessions
				.map((session) => ({
					...session,
					deltas: derived(api.deltas.Deltas({ projectId, sessionId: session.id }), (deltas) => {
						if (!fileFilter) return deltas;
						return Object.fromEntries(
							Object.entries(deltas).filter(([path]) => fileFilter.includes(path))
						);
					})
				}))
				.sort((a, b) => a.meta.startTimestampMs - b.meta.startTimestampMs);
		}
	);

	const scrollToSession = () => {
		const sessionEl = document.getElementById('current-session');
		if (sessionEl) {
			sessionEl.scrollIntoView({ behavior: 'smooth', block: 'center' });
		}
	};

	const currentSession = derived(
		[page, richSessions, data.currentSessionId],
		([page, sessions, currentSessionId]) =>
			sessions?.find((s) => s.id === currentSessionId) ??
			sessions?.find((s) => s.id === page.params.sessionId)
	);
	currentSession.subscribe(scrollToSession);

	const nextSessionId = derived([page, richSessions], ([page, sessions]) => {
		if (sessions) {
			const currentIndex = sessions.findIndex((s) => s.id === page.params.sessionId);
			if (currentIndex === -1) return undefined;
			if (currentIndex < sessions.length - 1) return sessions[currentIndex + 1].id;
			return undefined;
		}
	});

	const prevSessionId = derived([page, richSessions], ([page, sessions]) => {
		if (sessions) {
			const currentIndex = sessions.findIndex((s) => s.id === page.params.sessionId);
			if (currentIndex === -1) return undefined;
			if (currentIndex > 0) return sessions[currentIndex - 1].id;
			return undefined;
		}
	});

	const removeFromSearchParams = (params: URLSearchParams, key: string) => {
		params.delete(key);
		return params;
	};

	const getSessionURI = (sessionId: string) =>
		`/projects/${$page.params.projectId}/player/${
			$page.params.date
		}/${sessionId}?${removeFromSearchParams($page.url.searchParams, 'delta').toString()}`;

	let bookmarkModal: BookmarkModal;

	onMount(() =>
		unsubscribe(
			tinykeys(window, {
				'Shift+ArrowRight': () =>
					nextSessionId.load().then((sessionId) => {
						if (sessionId) goto(getSessionURI(sessionId));
					}),
				'Shift+ArrowLeft': () =>
					prevSessionId.load().then((sessionId) => {
						if (sessionId) goto(getSessionURI(sessionId));
					})
			}),

			events.on('openBookmarkModal', () => bookmarkModal?.show($currentTimestamp)),
			hotkeys.on('Meta+Shift+D', () => bookmarkModal?.show($currentTimestamp)),
			hotkeys.on('D', () =>
				api.bookmarks
					.upsert({
						projectId: $projectId,
						note: '',
						timestampMs: $currentTimestamp,
						deleted: false
					})
					.then(() => toasts.success('Bookmark created'))
			)
		)
	);
</script>

<article id="activities" class="card my-2 flex w-80 flex-shrink-0 flex-grow-0 flex-col xl:w-96">
	{#await richSessions.load()}
		<div class="flex h-full flex-col items-center justify-center">
			<div
				class="loader border-gray-200 mb-4 h-12 w-12 rounded-full border-4 border-t-4 ease-linear"
			/>
			<h2 class="text-center text-2xl font-medium text-gray-500">Loading...</h2>
		</div>
	{:then}
		<header
			class="card-header flex flex-row justify-between rounded-t border-b-[1px] border-b-divider bg-card-active px-3 py-2 leading-[21px]"
		>
			<div class="relative flex gap-2">
				<div class="relative bottom-[1px] h-4 w-4 text-sm">🧰</div>
				<div>Working History</div>
				<div class="text-zinc-400">
					{$richSessions.length}
				</div>
			</div>
		</header>

		<ul
			class="mr-1 flex h-full flex-col gap-2 overflow-auto rounded-b bg-card-default pt-2 pb-2 pl-2 pr-1"
		>
			{#each $richSessions as session}
				{@const isCurrent = session.id === $currentSession?.id}
				<SessionCard
					{isCurrent}
					{session}
					deltas={session.deltas}
					currentFilepath={$currentFilepath}
				/>
			{:else}
				<div class="mt-4 text-center text-zinc-300">No activities found</div>
			{/each}
		</ul>
	{/await}
</article>

<div id="player" class="card relative my-2 flex flex-auto flex-col overflow-auto">
	<header class="flex items-center gap-3 bg-card-active px-3 py-2">
		{#await Promise.all([currentSession.load(), nextSessionId.load(), prevSessionId.load()])}
			<span>Loading...</span>
		{:then}
			{#if !$currentSession}
				<span>No session found</span>
			{:else}
				<span class="min-w-[200px]">
					{format($currentSession.meta.startTimestampMs, 'EEEE, LLL d, HH:mm')}
					-
					{format($currentSession.meta.lastTimestampMs, 'HH:mm')}
				</span>
				<div class="flex items-center gap-1">
					<a
						href={$prevSessionId && getSessionURI($prevSessionId)}
						class="rounded border border-zinc-500 bg-zinc-600 p-0.5"
						class:hover:bg-zinc-500={!!$prevSessionId}
						class:pointer-events-none={!$prevSessionId}
						class:text-zinc-500={!$prevSessionId}
					>
						<IconChevronLeft class="h-4 w-4" />
					</a>
					<a
						href={$nextSessionId && getSessionURI($nextSessionId)}
						class="rounded border border-zinc-500 bg-zinc-600 p-0.5"
						class:hover:bg-zinc-500={!!$nextSessionId}
						class:pointer-events-none={!$nextSessionId}
						class:text-zinc-500={!$nextSessionId}
					>
						<IconChevronRight class="h-4 w-4" />
					</a>
				</div>
			{/if}
		{/await}
	</header>

	<slot />
</div>

<BookmarkModal bind:this={bookmarkModal} projectId={$projectId} />
