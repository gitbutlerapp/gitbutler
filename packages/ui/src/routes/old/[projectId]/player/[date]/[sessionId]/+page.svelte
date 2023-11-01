<script lang="ts">
	import Slider from './Slider.svelte';
	import type { PageData } from './$types';
	import { page } from '$app/stores';
	import { writable } from '@square/svelte-store';
	import Playback from './Playback.svelte';
	import type { Frame as FrameType } from './frame';
	import Frame from './Frame.svelte';
	import Info from './Info.svelte';
	import { getBookmarksStore } from '$lib/stores/bookmarks';

	export let data: PageData;
	const { currentFilepath, currentTimestamp, richSessions, currentSessionId } = data;

	let fullContext = true;
	let context = 8;

	$: bookmarks = getBookmarksStore({ projectId: $page.params.projectId });
	$: richSessionsState = richSessions?.state;

	$: currentDeltaIndex = parseInt($page.url.searchParams.get('delta') || '0');
	$: if ($page.params.sessionId) {
		currentSessionId.set($page.params.sessionId);
	}

	let frame: FrameType | null = null;

	$: if (frame) {
		currentSessionId.set(frame.sessionId);
		currentFilepath.set(frame.filepath);
		currentTimestamp.set(frame.deltas[frame?.deltas.length - 1].timestampMs);
	}

	const value = writable(0);

	$: if ($richSessions) {
		// this hook updates player value if current page url has changed
		const currentSessionIndex = $richSessions.findIndex((s) => {
			return s.id == $page.params.sessionId;
		});
		$value =
			$richSessions
				.filter((_, index) => index < currentSessionIndex)
				.reduce((acc, s) => {
					return acc + s.deltas.length;
				}, 0) + currentDeltaIndex;
	}
</script>

{#if $richSessionsState?.isLoading}
	<div class="flex h-full flex-col items-center justify-center">
		<div
			class="loader border-gray-200 mb-4 h-12 w-12 rounded-full border-4 border-t-4 ease-linear"
		/>
		<h2 class="text-center text-2xl font-medium text-gray-500">Loading...</h2>
	</div>
{:else if $richSessionsState?.isError}
	<div class="flex h-full flex-col items-center justify-center">
		<h2 class="text-center text-2xl font-medium text-gray-500">Something went wrong</h2>
	</div>
{:else}
	<Frame
		{context}
		{fullContext}
		sessions={$richSessions}
		deltas={$richSessions?.map((s) => s.deltas)}
		files={$richSessions?.map((s) => s.files)}
		bind:frame
		value={$value}
	/>

	{#if frame}
		<div id="info" class="floating absolute bottom-[86px] right-[9px]">
			<Info timestampMs={$currentTimestamp} filename={$currentFilepath} />
		</div>
	{/if}

	<div class="flex-shrink flex-grow"></div>
	<div
		id="controls"
		class="border-color-4 bg-color-3 bottom-0 flex w-full flex-col gap-4 rounded-bl rounded-br border-t p-2 pt-4"
	>
		<Slider sessions={$richSessions?.map(({ deltas }) => deltas)} {bookmarks} bind:value={$value} />
		<Playback
			deltas={$richSessions?.map(({ deltas }) => deltas)}
			bind:value={$value}
			bind:context
			bind:fullContext
		/>
	</div>
{/if}
