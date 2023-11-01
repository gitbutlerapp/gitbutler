<script lang="ts">
	import type { LayoutData } from './$types';
	import { page } from '$app/stores';
	import { onMount } from 'svelte';
	import * as hotkeys from '$lib/hotkeys';
	import * as events from '$lib/events';
	import BookmarkModal from './BookmarkModal.svelte';
	import { unsubscribe } from '$lib/utils';
	import SessionsList from './SessionsList.svelte';
	import SessionNavigations from './SessionNavigations.svelte';
	import { IconLoading } from '$lib/icons';
	import * as bookmarks from '$lib/api/bookmarks';
	import { goto } from '$app/navigation';

	export let data: LayoutData;
	const {
		currentSession,
		richSessions,
		richSessions2,
		currentSessionId,
		currentFilepath,
		currentTimestamp,
		projectId
	} = data;

	$: richSessionsState = richSessions.state;
	$: currentSessionsState = currentSession.state;

	let bookmarkModal: BookmarkModal;

	onMount(() =>
		unsubscribe(
			events.on('openBookmarkModal', () => bookmarkModal?.show($currentTimestamp)),
			hotkeys.on('Meta+Shift+D', () => bookmarkModal?.show($currentTimestamp)),
			hotkeys.on('Meta+Shift+R', () => goto(`/${projectId}/`)),
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
			sessions={$richSessions2}
			currentSessionId={$currentSessionId}
			currentFilepath={$currentFilepath}
		/>
	{/if}
</article>

<div id="player" class="card relative my-2 flex flex-auto flex-col overflow-auto">
	<header class="bg-color-3 flex items-center gap-3 px-3 py-2">
		{#if $currentSessionsState?.isLoading || $richSessionsState?.isLoading}
			<span>Loading...</span>
		{:else if $currentSessionsState?.isError || $richSessionsState?.isError}
			<span>Error</span>
		{:else if !$currentSession}
			<span>No session found</span>
		{:else}
			<SessionNavigations currentSession={$currentSession} sessions={$richSessions2} />
		{/if}
	</header>

	<slot />
</div>

<BookmarkModal bind:this={bookmarkModal} projectId={$page.params.projectId} />
