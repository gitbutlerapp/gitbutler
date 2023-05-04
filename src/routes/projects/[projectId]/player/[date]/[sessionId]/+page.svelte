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
	import { derived, writable } from 'svelte/store';
	import {
		IconChevronLeft,
		IconChevronRight,
		IconPlayerPauseFilled,
		IconPlayerPlayFilled
	} from '$lib/components/icons';
	import { collapse } from '$lib/paths';
	import { page } from '$app/stores';
	import { DeltasViewer, DiffContext } from '$lib/components';
	import { asyncDerived } from '@square/svelte-store';
	import { format } from 'date-fns';
	import { onMount } from 'svelte';
	import tinykeys from 'tinykeys';

	export let data: PageData;

	let fullContext = true;
	let context = 8;

	const unique = (value: any, index: number, self: any[]) => self.indexOf(value) === index;
	const lexically = (a: string, b: string) => a.localeCompare(b);

	const { sessions, projectId } = data;

	const richSessions = asyncDerived([sessions, page], async ([sessions, page]) => {
		const fileFilter = page.url.searchParams.get('file');
		const paths = fileFilter ? [fileFilter] : undefined;
		return Promise.all(
			sessions
				.filter(
					(session) => format(session.meta.startTimestampMs, 'yyyy-MM-dd') === page.params.date
				)
				.map((s) => enrichSession(projectId, s, paths))
		).then((sessions) =>
			sessions
				.filter((s) => s.deltas.length > 0)
				.sort((a, b) => a.meta.startTimestampMs - b.meta.startTimestampMs)
		);
	});

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

	const currentSession = derived([currentSessionId, richSessions], ([currentSessionId, sessions]) =>
		sessions?.find((session) => session.id === currentSessionId)
	);

	const hasNextSession = derived(
		[currentSessionId, richSessions],
		([currentSessionId, sessions]) =>
			sessions?.findIndex((session) => session.id === currentSessionId) < sessions?.length - 1
	);

	const hasPrevSession = derived(
		[currentSessionId, richSessions],
		([currentSessionId, sessions]) =>
			sessions?.findIndex((session) => session.id === currentSessionId) > 0
	);

	const frame = derived([currentSession, currentDeltaIndex], ([session, currentDeltaIndex]) => {
		if (!session) return null;
		const deltas = session.deltas.slice(0, currentDeltaIndex + 1);
		const filepath = deltas[deltas.length - 1][0];
		return {
			session,
			filepath,
			doc: session.files[filepath] || '',
			deltas: deltas.filter((delta) => delta[0] === filepath).map((delta) => delta[1])
		};
	});

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

	// scroller
	const maxInput = derived(richSessions, (sessions) =>
		sessions ? sessions.flatMap((session) => session.deltas).length : 0
	);

	const inputValue = writable(0);
	$: {
		if ($richSessions) {
			const currentSessionIndex = $richSessions.findIndex(
				(session) => session.id === $currentSessionId
			);
			if (currentSessionIndex > -1) {
				$inputValue =
					$richSessions.slice(0, currentSessionIndex).flatMap((session) => session.deltas).length +
					$currentDeltaIndex;
			}
		}
	}

	const goToNextSession = () => {
		if ($hasNextSession) {
			const currentSessionIndex = $richSessions.findIndex(
				(session) => session.id === $currentSessionId
			);
			const nextSession = $richSessions[currentSessionIndex + 1];
			currentSessionId.set(nextSession.id);
			currentDeltaIndex.set(0);
		}
	};

	const goToPrevSession = () => {
		if ($hasPrevSession) {
			const currentSessionIndex = $richSessions.findIndex(
				(session) => session.id === $currentSessionId
			);
			const prevSession = $richSessions[currentSessionIndex - 1];
			currentSessionId.set(prevSession.id);
			currentDeltaIndex.set(0);
		}
	};

	inputValue.subscribe((value) => {
		let i = 0;
		for (const session of $richSessions || []) {
			if (i < value && value < i + session.deltas.length) {
				currentSessionId.set(session.id);
				currentDeltaIndex.set(value - i);
				break;
			}
			i += session.deltas.length;
		}
	});

	// player
	let interval: ReturnType<typeof setInterval> | undefined;
	let direction: -1 | 1 = 1;
	let speed = 1;
	let oneSecond = 1000;
	$: isPlaying = !!interval;

	const stop = () => {
		clearInterval(interval);
		interval = undefined;
		speed = 1;
	};

	const play = () => start({ direction, speed });

	const start = (params: { direction: 1 | -1; speed: number }) => {
		if (interval) clearInterval(interval);
		interval = setInterval(() => {
			gotoNextDelta();
		}, oneSecond / params.speed);
	};

	const gotoNextDelta = () => {
		if ($inputValue < $maxInput) {
			$inputValue += 1;
		} else {
			stop();
		}
	};

	const removeFromSearchParams = (params: URLSearchParams, key: string) => {
		params.delete(key);
		return params;
	};

	const gotoPrevDelta = () => {
		if ($inputValue > 0) {
			$inputValue -= 1;
		} else {
			stop();
		}
	};

	const speedUp = () => {
		speed = speed * 2;
		start({ direction, speed });
	};

	onMount(() =>
		tinykeys(window, {
			ArrowRight: gotoNextDelta,
			'Shift+ArrowRight': goToNextSession,
			ArrowLeft: gotoPrevDelta,
			'Shift+ArrowLeft': goToPrevSession,
			Space: () => {
				if (isPlaying) {
					stop();
				} else {
					play();
				}
			}
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
					<a
						href="/projects/{projectId}/player/{$currentDate}/{session.id}?{removeFromSearchParams(
							$page.url.searchParams,
							'delta'
						).toString()}"
						class:pointer-events-none={isCurrent}
						class="w-full"
					>
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
										class:text-zinc-100={$frame?.filepath === filename}
										class:bg-[#3356C2]={$frame?.filepath === filename}
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
	{#if $frame}
		<header class="flex items-center gap-3 bg-card-active px-3 py-2">
			<span class="min-w-[200px]">
				{format($frame.session.meta.startTimestampMs, 'EEEE, LLL d, HH:mm')}
				-
				{format($frame.session.meta.lastTimestampMs, 'HH:mm')}
			</span>
			<div class="flex items-center gap-1">
				<button
					on:click={goToPrevSession}
					class="rounded border border-zinc-500 bg-zinc-600 p-0.5"
					class:hover:bg-zinc-500={$hasPrevSession}
					class:cursor-not-allowed={!$hasPrevSession}
					class:text-zinc-500={!$hasPrevSession}
				>
					<IconChevronLeft class="h-4 w-4" />
				</button>
				<button
					on:click={goToNextSession}
					class="rounded border border-zinc-500 bg-zinc-600 p-0.5"
					class:hover:bg-zinc-500={$hasNextSession}
					class:cursor-not-allowed={!$hasNextSession}
					class:text-zinc-500={!$hasNextSession}
				>
					<IconChevronRight class="h-4 w-4" />
				</button>
			</div>
		</header>
		<div id="code" class="flex-auto overflow-auto bg-[#1E2021]">
			<div class="pb-[200px]">
				<DeltasViewer
					doc={$frame.doc}
					deltas={$frame.deltas}
					filepath={$frame.filepath}
					paddingLines={fullContext ? 100000 : context}
				/>
			</div>
		</div>

		<div
			id="info"
			class="w-content absolute bottom-[86px] ml-2 flex max-w-full gap-2 rounded-full bg-zinc-900/80 py-2 px-4 shadow"
			style="
				border: 0.5px solid rgba(63, 63, 70, 0.5);
				-webkit-backdrop-filter: blur(5px) saturate(190%) contrast(70%) brightness(80%);
				background-color: rgba(1, 1, 1, 0.6);
			"
		>
			<span class="flex-auto overflow-auto font-mono text-[12px] text-zinc-300">
				{collapse($frame.filepath)}
			</span>
			<span class="whitespace-nowrap text-zinc-500">
				–
				{new Date($frame.deltas[$frame.deltas.length - 1].timestampMs).toLocaleString('en-US')}
			</span>
		</div>

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
			<Slider sessions={$richSessions} bind:value={$inputValue} />

			<div class="playback-controller-ui mx-auto flex w-full items-center justify-between gap-2">
				<div class="left-side flex space-x-8">
					<div class="play-button-button-container">
						{#if interval}
							<button
								class="player-button group fill-zinc-400 duration-300 ease-in-out hover:scale-125"
								on:click={stop}
							>
								<IconPlayerPauseFilled
									class="player-button-play icon-pointer h-6 w-6 fill-zinc-400 group-hover:fill-zinc-100 "
								/>
							</button>
						{:else}
							<button
								class="player-button group fill-zinc-400 duration-300 ease-in-out hover:scale-125"
								on:click={play}
							>
								<IconPlayerPlayFilled
									class="player-button-pause icon-pointer h-6 w-6 fill-zinc-400 group-hover:fill-zinc-100"
								/>
							</button>
						{/if}
					</div>

					<div class="back-forward-button-container ">
						<button
							on:click={gotoPrevDelta}
							class="player-button-back group duration-300 ease-in-out hover:scale-125"
						>
							<svg
								width="20"
								height="20"
								viewBox="0 0 20 20"
								fill="none"
								xmlns="http://www.w3.org/2000/svg"
								class="icon-pointer h-6 w-6"
							>
								<path
									fill-rule="evenodd"
									clip-rule="evenodd"
									d="M13.7101 16.32C14.0948 16.7047 14.0955 17.3274 13.7117 17.7111C13.3254 18.0975 12.7053 18.094 12.3206 17.7093L5.37536 10.7641C5.18243 10.5711 5.0867 10.32 5.08703 10.069C5.08802 9.81734 5.18374 9.56621 5.37536 9.37458L12.3206 2.42932C12.7055 2.04445 13.328 2.04396 13.7117 2.42751C14.0981 2.81386 14.0946 3.43408 13.7101 3.81863C13.4234 4.10528 7.80387 9.78949 7.52438 10.069C9.59011 12.1474 11.637 14.2469 13.7101 16.32Z"
									fill="none"
									class="fill-zinc-400 group-hover:fill-zinc-100"
								/>
							</svg>
						</button>

						<button
							on:click={gotoNextDelta}
							class="player-button-forward group duration-300 ease-in-out hover:scale-125"
						>
							<svg
								width="20"
								height="20"
								viewBox="0 0 20 20"
								fill="none"
								xmlns="http://www.w3.org/2000/svg"
								class="icon-pointer h-6 w-6"
							>
								<path
									fill-rule="evenodd"
									clip-rule="evenodd"
									d="M6.28991 16.32C5.90521 16.7047 5.90455 17.3274 6.28826 17.7111C6.67461 18.0975 7.29466 18.094 7.67938 17.7093L14.6246 10.7641C14.8176 10.5711 14.9133 10.32 14.913 10.069C14.912 9.81734 14.8163 9.56621 14.6246 9.37458L7.67938 2.42932C7.29451 2.04445 6.67197 2.04396 6.28826 2.42751C5.90192 2.81386 5.90537 3.43408 6.28991 3.81863C6.57656 4.10528 12.1961 9.78949 12.4756 10.069C10.4099 12.1474 8.36301 14.2469 6.28991 16.32Z"
									fill="none"
									class="fill-zinc-400 group-hover:fill-zinc-100"
								/>
							</svg>
						</button>
					</div>

					<button on:click={speedUp}>{speed}x</button>
				</div>

				<DiffContext bind:lines={context} bind:fullContext />
			</div>
		</div>
	{:else}
		<div class="mt-8 text-center">Select a playlist</div>
	{/if}
</div>
