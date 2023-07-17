<script lang="ts">
	import Slider from './Slider.svelte';
	import type { PageData } from './$types';
	import { page } from '$app/stores';
	import { get, writable } from '@square/svelte-store';
	import { derived, Loaded } from 'svelte-loadable-store';
	import { format } from 'date-fns';
	import { stores } from '$lib';
	import Playback from './Playback.svelte';
	import type { Frame as FrameType } from './frame';
	import Frame from './Frame.svelte';
	import Info from './Info.svelte';
	import type { Delta } from '$lib/api';

	export let data: PageData;
	const { currentFilepath, currentTimestamp, currentSessionId } = data;

	let fullContext = true;
	let context = 8;

	page.subscribe((page) => {
		currentDeltaIndex = parseInt(page.url.searchParams.get('delta') || '0');
		currentSessionId.set(page.params.sessionId);
	});

	const filter = derived(page, (page) => page.url.searchParams.get('file'));
	const projectId = derived(page, (page) => page.params.projectId);

	$: bookmarks = stores.bookmarks.list({ projectId: $page.params.projectId });
	$: sessions = stores.sessions({ projectId: $page.params.projectId });
	$: dateSessions = derived([sessions, page], ([sessions, page]) =>
		sessions?.filter(
			(session) => format(session.meta.startTimestampMs, 'yyyy-MM-dd') === page.params.date
		)
	);

	$: richSessions = derived([dateSessions, filter, projectId], ([sessions, filter, projectId]) =>
		sessions.map((session) => ({
			...session,
			deltas: derived(stores.deltas({ projectId: projectId, sessionId: session.id }), (deltas) =>
				Object.entries(deltas)
					.filter(([path]) => (filter ? path === filter : true))
					.flatMap(([path, deltas]) => deltas.map((delta) => [path, delta] as [string, Delta]))
					.sort((a, b) => a[1].timestampMs - b[1].timestampMs)
			),
			files: derived(stores.files({ projectId, sessionId: session.id }), (files) =>
				Object.fromEntries(
					Object.entries(files).filter(([path]) => (filter ? path === filter : true))
				)
			)
		}))
	);

	$: currentDeltaIndex = parseInt($page.url.searchParams.get('delta') || '0');

	richSessions?.subscribe((sessions) => {
		if (sessions.isLoading) return;
		if (Loaded.isError(sessions)) return;
		if (sessions.value.length === 0) return;
		if (!sessions.value.some((s) => s.id === $currentSessionId)) {
			$currentSessionId = sessions.value[0].id;
		}
	});

	let frame: FrameType | null = null;

	$: if (frame) {
		currentSessionId.set(frame.sessionId);
		currentFilepath.set(frame.filepath);
		currentTimestamp.set(frame.deltas[frame?.deltas.length - 1].timestampMs);
	}

	const value = writable(0);

	$: {
		// this hook updates player value if current page url has changed
		if (!$richSessions.isLoading && Loaded.isValue($richSessions)) {
			const currentSessionIndex = $richSessions.value.findIndex(
				(s) => s.id === $page.params.sessionId
			);
			$value =
				$richSessions.value
					.filter((_, index) => index < currentSessionIndex)
					.reduce((acc, s) => {
						const deltas = get(s.deltas);
						if (!deltas.isLoading && Loaded.isValue(deltas)) {
							return acc + deltas.value.length;
						} else {
							return acc;
						}
					}, 0) + currentDeltaIndex;
		}
	}
</script>

{#if $richSessions.isLoading}
	<div class="flex h-full flex-col items-center justify-center">
		<div
			class="loader border-gray-200 mb-4 h-12 w-12 rounded-full border-4 border-t-4 ease-linear"
		/>
		<h2 class="text-center text-2xl font-medium text-gray-500">Loading...</h2>
	</div>
{:else if Loaded.isError($richSessions)}
	<div class="flex h-full flex-col items-center justify-center">
		<h2 class="text-center text-2xl font-medium text-gray-500">Something went wrong</h2>
	</div>
{:else}
	<Frame
		{context}
		{fullContext}
		sessions={$richSessions.value}
		deltas={derived(
			$richSessions.value.map(({ deltas }) => deltas),
			(deltas) => deltas
		)}
		files={derived(
			$richSessions.value.map(({ files }) => files),
			(files) => files
		)}
		bind:frame
		value={$value}
	/>

	{#if frame}
		<div id="info" class="floating absolute bottom-[86px] right-[9px]">
			<Info timestampMs={$currentTimestamp} filename={$currentFilepath} />
		</div>
	{/if}

	<div
		id="controls"
		class="absolute bottom-0 flex w-full flex-col gap-4 overflow-hidden rounded-bl rounded-br border-t border-zinc-700 bg-[#2E2E32]/75 p-2 pt-4"
		style="
            border-width: 0.5px;
            -webkit-backdrop-filter: blur(5px) saturate(190%) contrast(70%) brightness(80%);
            backdrop-filter: blur(5px) saturate(190%) contrast(70%) brightness(80%);
            background-color: rgba(24, 24, 27, 0.60);
            border: 0.5px solid rgba(63, 63, 70, 0.50);
        "
	>
		<Slider
			sessions={derived(
				$richSessions.value.map(({ deltas }) => deltas),
				(deltas) => deltas
			)}
			{bookmarks}
			bind:value={$value}
		/>
		<Playback
			deltas={derived(
				$richSessions.value.map(({ deltas }) => deltas),
				(deltas) => deltas
			)}
			bind:value={$value}
			bind:context
			bind:fullContext
		/>
	</div>
{/if}
