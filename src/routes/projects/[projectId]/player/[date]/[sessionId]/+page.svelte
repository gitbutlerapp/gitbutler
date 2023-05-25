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
	import { DeltasViewer } from '$lib/components';
	import { asyncDerived, derived, writable } from '@square/svelte-store';
	import { format } from 'date-fns';
	import { stores } from '$lib';
	import Info from './Info.svelte';
	import Playback from './Playback.svelte';

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

	$: currentSession = $richSessions?.find((s) => s.id === $currentSessionId);

	$: frameDeltas = currentSession?.deltas.slice(0, currentDeltaIndex + 1);
	$: frameFilepath = frameDeltas?.[frameDeltas.length - 1]?.[0];
	$: frame =
		!currentSession || !frameFilepath || !frameDeltas
			? null
			: {
					session: currentSession,
					filepath: frameFilepath,
					doc: currentSession.files[frameFilepath] || '',
					deltas: frameDeltas.filter((delta) => delta[0] === frameFilepath).map((delta) => delta[1])
			  };

	$: if (frame) currentFilepath.set(frame?.filepath);

	$: currentDelta = frame?.deltas[frame?.deltas.length - 1];
	$: {
		const timestamp = currentDelta?.timestampMs;
		if (timestamp) {
			currentTimestamp.set(timestamp);
		}
	}

	const value = writable(0);

	$: {
		if ($richSessions && currentSession) {
			const currentSessionIndex = $richSessions.findIndex((s) => s.id === currentSession?.id);
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
	{#if !frame}
		<div class="mt-8 text-center">Select a playlist</div>
	{:else}
		<div id="code" class="flex-auto overflow-auto bg-[#1E2021]">
			<div class="pb-[200px]">
				<DeltasViewer
					doc={frame.doc}
					deltas={frame.deltas}
					filepath={frame.filepath}
					paddingLines={fullContext ? 100000 : context}
				/>
			</div>
		</div>

		{#if currentDelta}
			<div id="info" class="floating absolute bottom-[86px] right-[9px]">
				<Info
					projectId={$projectId}
					timestampMs={currentDelta.timestampMs}
					filename={frame.filepath}
				/>
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
	{/if}
{/await}
