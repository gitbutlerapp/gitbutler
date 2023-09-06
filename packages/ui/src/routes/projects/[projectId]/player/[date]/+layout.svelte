<script lang="ts">
	import type { LayoutData } from './$types';
	import { page } from '$app/stores';
	import { format } from 'date-fns';
	import { onMount } from 'svelte';
	import * as hotkeys from '$lib/hotkeys';
	import * as events from '$lib/events';
	import BookmarkModal from './BookmarkModal.svelte';
	import { unsubscribe } from '$lib/utils';
	import SessionsList from './SessionsList.svelte';
	import SessionNavigations from './SessionNavigations.svelte';
	import { IconLoading } from '$lib/icons';
	import { getSessionStore } from '$lib/stores/sessions';
	import { getDeltasStore } from '$lib/stores/deltas';
	import { getFilesStore } from '$lib/stores/files';
	import * as bookmarks from '$lib/api/ipc/bookmarks';
	import { asyncDerived } from '@square/svelte-store';
	import { derived } from 'svelte/store';

	export let data: LayoutData;
	const { currentFilepath, currentTimestamp } = data;

	const filter = derived(page, (page) => page.url.searchParams.get('file'));
	const projectId = derived(page, (page) => page.params.projectId);

	$: sessions = getSessionStore({ projectId: $page.params.projectId });
	$: dateSessions = asyncDerived([sessions, page], async ([sessions, page]) =>
		sessions
			.filter((session) => format(session.meta.startTimestampMs, 'yyyy-MM-dd') === page.params.date)
			.sort((a, b) => a.meta.startTimestampMs - b.meta.startTimestampMs)
	);

	$: richSessions = asyncDerived(
		[dateSessions, projectId, filter],
		async ([sessions, projectId, filter]) =>
			sessions.map((session) => ({
				...session,
				deltas: asyncDerived(
					getDeltasStore({ projectId: projectId, sessionId: session.id }),
					async (deltas) =>
						Object.fromEntries(
							Object.entries(deltas).filter(([path]) => (filter ? path === filter : true))
						)
				),
				files: asyncDerived(
					getFilesStore({ projectId: projectId, sessionId: session.id }),
					async (files) =>
						Object.fromEntries(
							Object.entries(files).filter(([path]) => (filter ? path === filter : true))
						)
				)
			}))
	);
	$: richSessionsState = richSessions.state;

	$: currentSession = asyncDerived(
		[page, richSessions, data.currentSessionId],
		async ([page, sessions, currentSessionId]) =>
			sessions.find((s) => s.id === currentSessionId) ??
			sessions.find((s) => s.id === page.params.sessionId),
		{ trackState: true }
	);
	$: currentSessionsState = sessions.state;

	let bookmarkModal: BookmarkModal;

	onMount(() =>
		unsubscribe(
			events.on('openBookmarkModal', () => bookmarkModal?.show($currentTimestamp)),
			hotkeys.on('Meta+Shift+D', () => bookmarkModal?.show($currentTimestamp)),
			hotkeys.on('D', async () => {
				const existing = await bookmarks.list({
					projectId: $page.params.projectId,
					range: {
						start: $currentTimestamp,
						end: $currentTimestamp + 1
					}
				});
				const newBookmark =
					existing.length === 0
						? {
								projectId: $page.params.projectId,
								note: '',
								timestampMs: $currentTimestamp,
								deleted: false
						  }
						: {
								...existing[0],
								deleted: !existing[0].deleted
						  };
				bookmarks.upsert(newBookmark);
			})
		)
	);
</script>

<article id="activities" class="card my-2 flex w-80 flex-shrink-0 flex-grow-0 flex-col xl:w-96">
	{#if $richSessionsState?.isLoading || $currentSessionsState?.isLoading}
		<div class="flex h-full flex-col items-center justify-center">
			<IconLoading class="mb-4 h-12 w-12 animate-spin ease-linear" />
			<h2 class="text-center text-2xl font-medium text-gray-500">Loading...</h2>
		</div>
	{:else if $richSessionsState?.isError || $currentSessionsState?.isError}
		<div class="flex h-full flex-col items-center justify-center">
			<h2 class="text-center text-2xl font-medium text-gray-500">Error</h2>
		</div>
	{:else}
		<SessionsList
			sessions={$richSessions}
			currentSession={$currentSession}
			currentFilepath={$currentFilepath}
		/>
	{/if}
</article>

<div id="player" class="card relative my-2 flex flex-auto flex-col overflow-auto">
	<header class="flex items-center gap-3 bg-card-active px-3 py-2">
		{#if $currentSessionsState?.isLoading || $richSessionsState?.isLoading}
			<span>Loading...</span>
		{:else if $currentSessionsState?.isError || $richSessionsState?.isError}
			<span>Error</span>
		{:else if !$currentSession}
			<span>No session found</span>
		{:else}
			<SessionNavigations currentSession={$currentSession} sessions={$richSessions} />
		{/if}
	</header>

	<slot />
</div>

<BookmarkModal bind:this={bookmarkModal} projectId={$page.params.projectId} />
