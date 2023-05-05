<script lang="ts" context="module">
	import { deltas, type Session, type Delta } from '$lib/api';
	const enrichSession = async (projectId: string, session: Session, paths?: string[]) => {
		const sessionDeltas = await deltas
			.list({ projectId, sessionId: session.id, paths })
			.then((deltas) =>
				Object.entries(deltas)
					.flatMap(([path, deltas]) => deltas.map((delta) => [path, delta] as [string, Delta]))
					.sort((a, b) => a[1].timestampMs - b[1].timestampMs)
			);
		return {
			...session,
			deltas: sessionDeltas
		};
	};
</script>

<script lang="ts">
	import type { LayoutData } from './$types';
	import { IconChevronLeft, IconChevronRight } from '$lib/components/icons';
	import { collapse } from '$lib/paths';
	import { page } from '$app/stores';
	import { asyncDerived, derived, writable } from '@square/svelte-store';
	import { format } from 'date-fns';
	import { onMount } from 'svelte';
	import tinykeys from 'tinykeys';
	import { goto } from '$app/navigation';

	export let data: LayoutData;
	const { currentFilepath } = data;

	const unique = (value: any, index: number, self: any[]) => self.indexOf(value) === index;
	const lexically = (a: string, b: string) => a.localeCompare(b);

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
			const paths = fileFilter ? [fileFilter] : undefined;
			const richSessions = await Promise.all(
				sessions.map((s) => enrichSession(projectId, s, paths))
			);
			return richSessions
				.filter((s) => s.deltas.length > 0)
				.sort((a, b) => a.meta.startTimestampMs - b.meta.startTimestampMs);
		}
	);

	const currentDeltaIndex = writable(parseInt($page.url.searchParams.get('delta') || '0'));
	const currentSessionId = writable($page.params.sessionId);
	const currentDate = writable($page.params.date);

	richSessions.subscribe((sessions) => {
		if (!sessions) return;
		if (sessions.length === 0) return;
		if (!sessions.some((s) => s.id === $currentSessionId)) {
			currentSessionId.set(sessions[0].id);
		}
	});

	const scrollToSession = () => {
		const sessionEl = document.getElementById('current-session');
		if (sessionEl) {
			sessionEl.scrollIntoView({ behavior: 'smooth', block: 'center' });
		}
		const changedLines = document.getElementsByClassName('line-changed');
		if (changedLines.length > 0) {
			changedLines[0].scrollIntoView({ behavior: 'smooth', block: 'center' });
		}
	};

	currentSessionId.subscribe(scrollToSession);

	page.subscribe((page) => {
		currentDeltaIndex.set(parseInt(page.url.searchParams.get('delta') || '0'));
		currentSessionId.set(page.params.sessionId);
		currentDate.set(page.params.date);
	});

	const currentSessionIndex = derived(
		[currentSessionId, richSessions],
		([currentSessionId, sessions]) =>
			sessions?.findIndex((session) => session.id === currentSessionId)
	);

	const currentSession = derived(
		[currentSessionIndex, richSessions],
		([currentSessionIndex, sessions]) =>
			currentSessionIndex > -1 ? sessions[currentSessionIndex] : null
	);

	const nextSession = derived(
		[currentSessionIndex, richSessions],
		([currentSessionIndex, sessions]) =>
			currentSessionIndex < sessions?.length - 1 ? sessions[currentSessionIndex + 1] : null
	);

	const prevSession = derived(
		[currentSessionIndex, richSessions],
		([currentSessionIndex, sessions]) =>
			currentSessionIndex > 0 ? sessions[currentSessionIndex - 1] : null
	);

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

	const sessionDuration = (session: Session) =>
		`${Math.round((session.meta.lastTimestampMs - session.meta.startTimestampMs) / 1000 / 60)} min`;

	const removeFromSearchParams = (params: URLSearchParams, key: string) => {
		params.delete(key);
		return params;
	};

	const getSessionURI = (sessionId: string) =>
		`/projects/${$page.params.projectId}/player/${
			$page.params.date
		}/${sessionId}?${removeFromSearchParams($page.url.searchParams, 'delta').toString()}`;

	onMount(() =>
		tinykeys(window, {
			'Shift+ArrowRight': () =>
				nextSession.load().then((session) => session && goto(getSessionURI(session.id))),
			'Shift+ArrowLeft': () =>
				prevSession.load().then((session) => session && goto(getSessionURI(session.id)))
		})
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
				{@const isCurrent = session.id === $currentSessionId}
				{@const filesChagned = new Set(session.deltas.map(([path]) => path)).size}
				<li
					id={isCurrent ? 'current-session' : ''}
					class:bg-card-active={isCurrent}
					class="session-card rounded border-[0.5px] border-gb-700 text-zinc-300 shadow-md"
				>
					<a href={getSessionURI(session.id)} class:pointer-events-none={isCurrent} class="w-full">
						<div class="flex flex-row justify-between rounded-t px-3 pt-3">
							<span>{sessionRange(session)}</span>
							<span>{sessionDuration(session)}</span>
						</div>

						<span class="flex flex-row justify-between px-3 pb-3">
							{filesChagned}
							{filesChagned !== 1 ? 'files' : 'file'}
						</span>

						{#if isCurrent}
							<ul
								class="list-disk list-none overflow-hidden rounded-bl rounded-br bg-zinc-800 py-1 pl-0 pr-2"
								style:list-style="disc"
							>
								{#each session.deltas
									.map((d) => d[0])
									.filter(unique)
									.sort(lexically) as filename}
									<li
										class:text-zinc-100={$currentFilepath === filename}
										class:bg-[#3356C2]={$currentFilepath === filename}
										class="mx-5 ml-1 w-full list-none rounded p-1 text-zinc-500"
									>
										{collapse(filename)}
									</li>
								{/each}
							</ul>
						{/if}
					</a>
				</li>
			{:else}
				<div class="mt-4 text-center text-zinc-300">No activities found</div>
			{/each}
		</ul>
	{/await}
</article>

<div id="player" class="card relative my-2 flex flex-auto flex-col overflow-auto">
	<header class="flex items-center gap-3 bg-card-active px-3 py-2">
		{#await Promise.all([currentSession.load(), nextSession.load(), prevSession.load()])}
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
						href={$prevSession && getSessionURI($prevSession.id)}
						class="rounded border border-zinc-500 bg-zinc-600 p-0.5"
						class:hover:bg-zinc-500={!!$prevSession}
						class:pointer-events-none={!$prevSession}
						class:text-zinc-500={!$prevSession}
					>
						<IconChevronLeft class="h-4 w-4" />
					</a>
					<a
						href={$nextSession && getSessionURI($nextSession.id)}
						class="rounded border border-zinc-500 bg-zinc-600 p-0.5"
						class:hover:bg-zinc-500={!!$nextSession}
						class:pointer-events-none={!$nextSession}
						class:text-zinc-500={!$nextSession}
					>
						<IconChevronRight class="h-4 w-4" />
					</a>
				</div>
			{/if}
		{/await}
	</header>

	<slot />
</div>
