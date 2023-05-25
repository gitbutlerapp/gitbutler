<script lang="ts" context="module">
	import { deltas, files, type Session, type Delta } from '$lib/api';
	const enrichSession = async (projectId: string, session: Session, paths?: string[]) => {
		const sessionFiles = await files.list({ projectId, sessionId: session.id, paths });
		const sessionDeltas = await deltas
			.list({ projectId, sessionId: session.id, paths })
			.then((deltas) =>
				Object.entries(deltas)
					.flatMap(([path, deltas]) => deltas.map((delta) => [path, delta] as [string, Delta]))
					.sort((a, b) => a[1].timestampMs - b[1].timestampMs)
			);
		const deltasFiles = new Set(sessionDeltas.map(([path]) => path));
		return {
			...session,
			files: Object.fromEntries(
				Object.entries(sessionFiles).filter(([filepath]) => deltasFiles.has(filepath))
			),
			deltas: sessionDeltas
		};
	};
</script>

<script lang="ts">
	import Slider from './Slider.svelte';
	import type { PageData } from './$types';
	import { page } from '$app/stores';
	import { asyncDerived, derived, writable } from '@square/svelte-store';
	import { format } from 'date-fns';
	import { stores } from '$lib';
	import Playback from './Playback.svelte';
	import type { Frame as FrameType } from './frame';
	import Frame from './Frame.svelte';
	import Info from './Info.svelte';

	export let data: PageData;
	const { currentFilepath, currentTimestamp, currentSessionId } = data;

	let fullContext = true;
	let context = 8;

	const dateSessions = derived([data.sessions, page], ([sessions, page]) =>
		sessions?.filter(
			(session) => format(session.meta.startTimestampMs, 'yyyy-MM-dd') === page.params.date
		)
	);

	page.subscribe((page) => {
		currentDeltaIndex = parseInt(page.url.searchParams.get('delta') || '0');
		currentSessionId.set(page.params.sessionId);
	});

	const fileFilter = derived(page, (page) => page.url.searchParams.get('file'));
	const projectId = derived(page, (page) => page.params.projectId);

	$: bookmarks = stores.bookmarks.list({ projectId: $projectId });

	const richSessions = asyncDerived(
		[dateSessions, fileFilter, projectId],
		async ([sessions, fileFilter, projectId]) => {
			const paths = fileFilter ? [fileFilter] : undefined;
			const richSessions = await Promise.all(
				sessions.map((s) => enrichSession(projectId, s, paths))
			);
			return richSessions
				.filter((s) => s.deltas.length > 0)
				.sort((a, b) => a.meta.startTimestampMs - b.meta.startTimestampMs);
		}
	);

	$: currentDeltaIndex = parseInt($page.url.searchParams.get('delta') || '0');

	richSessions.subscribe((sessions) => {
		if (!sessions) return;
		if (sessions.length === 0) return;
		if (!sessions.some((s) => s.id === $currentSessionId)) {
			$currentSessionId = sessions[0].id;
		}
	});

	let frame: FrameType | null = null;

	$: if (frame) {
		currentFilepath.set(frame.filepath);
		currentTimestamp.set(frame.deltas[frame?.deltas.length - 1].timestampMs);
	}

	const value = writable(0);

	$: {
		if ($richSessions) {
			const currentSessionIndex = $richSessions.findIndex((s) => s.id === $currentSessionId);
			$value =
				$richSessions
					.filter((_, index) => index < currentSessionIndex)
					.reduce((acc, s) => acc + s.deltas.length, 0) + currentDeltaIndex;
		}
	}

	value.subscribe((value) => {
		let i = 0;
		for (const session of $richSessions || []) {
			if (i < value && value < i + session.deltas.length) {
				$currentSessionId = session.id;
				currentDeltaIndex = value - i;
				break;
			}
			i += session.deltas.length;
		}
	});
</script>

{#await richSessions.load()}
	<div class="flex h-full flex-col items-center justify-center">
		<div
			class="loader border-gray-200 mb-4 h-12 w-12 rounded-full border-4 border-t-4 ease-linear"
		/>
		<h2 class="text-center text-2xl font-medium text-gray-500">Loading...</h2>
	</div>
{:then}
	<Frame
		{context}
		{fullContext}
		deltas={$richSessions.map(({ deltas }) => deltas)}
		files={$richSessions.map(({ files }) => files)}
		bind:frame
		value={$value}
	/>

	{#if frame}
		<div id="info" class="floating absolute bottom-[86px] right-[9px]">
			<Info projectId={$projectId} timestampMs={$currentTimestamp} filename={$currentFilepath} />
		</div>
	{/if}

	<div
		id="controls"
		class="absolute bottom-0 flex w-full flex-col gap-4 overflow-hidden rounded-br rounded-bl border-t border-zinc-700 bg-[#2E2E32]/75 p-2 pt-4"
		style="
            border-width: 0.5px;
            -webkit-backdrop-filter: blur(5px) saturate(190%) contrast(70%) brightness(80%);
            backdrop-filter: blur(5px) saturate(190%) contrast(70%) brightness(80%);
            background-color: rgba(24, 24, 27, 0.60);
            border: 0.5px solid rgba(63, 63, 70, 0.50);
        "
	>
		<Slider sessions={$richSessions} bookmarks={$bookmarks} bind:value={$value} />

		<Playback
			deltas={$richSessions.map(({ deltas }) => deltas)}
			bind:value={$value}
			bind:context
			bind:fullContext
		/>
	</div>
{/await}
