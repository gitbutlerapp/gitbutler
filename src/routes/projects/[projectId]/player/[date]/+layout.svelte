<script lang="ts">
	import type { LayoutData } from './$types';
	import { page } from '$app/stores';
	import { derived, Loaded } from 'svelte-loadable-store';
	import { format } from 'date-fns';
	import { onMount } from 'svelte';
	import { events, hotkeys } from '$lib';
	import BookmarkModal from './BookmarkModal.svelte';
	import { unsubscribe } from '$lib/utils';
	import SessionsList from './SessionsList.svelte';
	import SessionNavigations from './SessionNavigations.svelte';
	import { IconLoading } from '$lib/icons';
	import { getSessionStore } from '$lib/stores/sessions';
	import { getDeltasStore } from '$lib/stores/deltas';
	import { getFilesStore } from '$lib/stores/files';
	import * as bookmarks from '$lib/api/ipc/bookmarks';

	export let data: LayoutData;
	const { currentFilepath, currentTimestamp } = data;

	const filter = derived(page, (page) => page.url.searchParams.get('file'));
	const projectId = derived(page, (page) => page.params.projectId);

	$: sessions = getSessionStore({ projectId: $page.params.projectId });
	$: dateSessions = derived([sessions, page], ([sessions, page]) =>
		sessions
			.filter((session) => format(session.meta.startTimestampMs, 'yyyy-MM-dd') === page.params.date)
			.sort((a, b) => a.meta.startTimestampMs - b.meta.startTimestampMs)
	);

	$: richSessions = derived([dateSessions, projectId, filter], ([sessions, projectId, filter]) =>
		sessions.map((session) => ({
			...session,
			deltas: derived(getDeltasStore({ projectId: projectId, sessionId: session.id }), (deltas) =>
				Object.fromEntries(
					Object.entries(deltas).filter(([path]) => (filter ? path === filter : true))
				)
			),
			files: derived(getFilesStore({ projectId, sessionId: session.id }), (files) =>
				Object.fromEntries(
					Object.entries(files).filter(([path]) => (filter ? path === filter : true))
				)
			)
		}))
	);

	$: currentSession = derived(
		[page, richSessions, data.currentSessionId],
		([page, sessions, currentSessionId]) =>
			sessions.find((s) => s.id === currentSessionId) ??
			sessions.find((s) => s.id === page.params.sessionId)
	);

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
	{#if $richSessions.isLoading || $currentSession.isLoading}
		<div class="flex h-full flex-col items-center justify-center">
			<IconLoading class="mb-4 h-12 w-12 animate-spin ease-linear" />
			<h2 class="text-center text-2xl font-medium text-gray-500">Loading...</h2>
		</div>
	{:else if Loaded.isError($richSessions) || Loaded.isError($currentSession)}
		<div class="flex h-full flex-col items-center justify-center">
			<h2 class="text-center text-2xl font-medium text-gray-500">Error</h2>
		</div>
	{:else}
		<SessionsList
			sessions={$richSessions.value}
			currentSession={$currentSession.value}
			currentFilepath={$currentFilepath}
		/>
	{/if}
</article>

<div id="player" class="card relative my-2 flex flex-auto flex-col overflow-auto">
	<header class="flex items-center gap-3 bg-card-active px-3 py-2">
		{#if $currentSession.isLoading || $richSessions.isLoading}
			<span>Loading...</span>
		{:else if Loaded.isError($currentSession) || Loaded.isError($richSessions)}
			<span>Error</span>
		{:else if !$currentSession.value}
			<span>No session found</span>
		{:else}
			<SessionNavigations currentSession={$currentSession.value} sessions={$richSessions.value} />
		{/if}
	</header>

	<slot />
</div>

<BookmarkModal bind:this={bookmarkModal} projectId={$page.params.projectId} />
