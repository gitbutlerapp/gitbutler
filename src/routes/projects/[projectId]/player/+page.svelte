<script lang="ts">
	import type { PageData } from './$types';
	import { listFiles } from '$lib/sessions';
	import { type Delta, list as listDeltas, Operation } from '$lib/deltas';
	import { CodeViewer } from '$lib/components';
	import { IconPlayerPauseFilled, IconPlayerPlayFilled } from '$lib/components/icons';
	import slider from '$lib/slider';
	import { onMount } from 'svelte';

	export let data: PageData;

	const { sessions } = data;

	let currentPlayerValue = 0;
	let currentDay = dateToYmd(new Date());

	$: sessionDays = $sessions.reduce((group, session) => {
		const day = dateToYmd(new Date(session.meta.startTimestampMs));
		group[day] = group[day] ?? [];
		group[day].push(session);
		// sort by startTimestampMs
		group[day].sort((a, b) => a.meta.startTimestampMs - b.meta.startTimestampMs);
		return group;
	}, {});

	$: currentSessions = $sessions.filter((session) => {
		let sessionDay = dateToYmd(new Date(session.meta.startTimestampMs));
		return sessionDay === currentDay;
	});

	let currentDeltas: Record<string, Promise<Record<string, Delta[]>>> = {};
	$: currentSessions
		.filter((s) => currentDeltas[s.id] === undefined)
		.forEach((s) => {
			currentDeltas[s.id] = listDeltas({
				projectId: data.projectId,
				sessionId: s.id
			});
		});

	type VideoFileEdit = {
		filepath: string;
		delta: Delta;
	};

	type VideoChapter = {
		title: string;
		session: string;
		files: Record<string, number>;
		edits: VideoFileEdit[];
		editCount: number;
		firstDeltaTimestampMs: number;
		lastDeltaTimestampMs: number;
		totalDurationMs: number;
	};

	type DayVideo = {
		chapters: VideoChapter[];
		editCount: number;
		totalDurationMs: number;
		firstDeltaTimestampMs: number;
		lastDeltaTimestampMs: number;
	};

	let dayPlaylist: Record<string, DayVideo> = {};
	let sessionFiles: Record<string, Record<string, string>> = {};
	let sessionChapters: Record<string, VideoChapter> = {};

	$: currentSessions.forEach((s) => listSession(s.id));

	function listSession(sid: string) {
		console.log('session', sid);

		currentDeltas[sid].then((deltas) => {
			if (sessionChapters[sid] === undefined) {
				sessionChapters[sid] = {
					title: sid,
					session: sid,
					files: {},
					edits: [],
					editCount: 0,
					firstDeltaTimestampMs: 9999999999999,
					lastDeltaTimestampMs: 0,
					totalDurationMs: 0
				};
			}
			sessionChapters[sid].edits = [];

			Object.entries(deltas).forEach(([filepath, deltas]) => {
				deltas.forEach((delta) => {
					sessionChapters[sid].edits.push({
						filepath,
						delta
					});
				});
				if (sessionFiles[sid] === undefined) sessionFiles[sid] = {};
				sessionFiles[sid][filepath] = '';
				sessionChapters[sid].editCount = sessionChapters[sid].edits.length;
				sessionChapters[sid].files[filepath] = deltas.length;
				sessionChapters[sid].firstDeltaTimestampMs = min(
					deltas.at(0)!.timestampMs,
					sessionChapters[sid].firstDeltaTimestampMs
				);
				sessionChapters[sid].lastDeltaTimestampMs = max(
					deltas.at(-1)!.timestampMs,
					sessionChapters[sid].lastDeltaTimestampMs
				);
				sessionChapters[sid].totalDurationMs =
					sessionChapters[sid].lastDeltaTimestampMs - sessionChapters[sid].firstDeltaTimestampMs;
			});

			console.log('sessionChapters', sessionChapters[sid]);
			// get the session chapters that are in the current day
			let dayChapters = Object.entries(sessionChapters)
				.filter(([sid, chapter]) => {
					let chapterDay = dateToYmd(new Date(chapter.firstDeltaTimestampMs));
					return chapterDay === currentDay;
				})
				.map(([, chapter]) => chapter)
				.sort((a, b) => a.firstDeltaTimestampMs - b.firstDeltaTimestampMs);

			dayPlaylist[currentDay] = {
				chapters: dayChapters,
				editCount: dayChapters.reduce((acc, chapter) => acc + chapter.editCount, 0),
				totalDurationMs: Object.values(dayChapters).reduce(
					(acc, chapter) => acc + chapter.totalDurationMs,
					0
				),
				firstDeltaTimestampMs: Object.values(dayChapters).reduce(
					(acc, chapter) => min(acc, chapter.firstDeltaTimestampMs),
					9999999999999
				),
				lastDeltaTimestampMs: Object.values(dayChapters).reduce(
					(acc, chapter) => max(acc, chapter.lastDeltaTimestampMs),
					0
				)
			};
			console.log('dayPlaylist', dayPlaylist);
		});

		listFiles({
			projectId: data.projectId,
			sessionId: sid
		}).then((files) => {
			Object.entries(sessionFiles[sid]).forEach(([filepath, _]) => {
				if (files[filepath] === undefined) {
					console.log('file not found', filepath);
				} else {
					sessionFiles[sid][filepath] = files[filepath];
					console.log('file found', sid, filepath, files[filepath]);
				}
			});
		});
	}

	function max<T>(a: T, b: T): T {
		return a > b ? a : b;
	}

	function min<T>(a: T, b: T): T {
		return a < b ? a : b;
	}

	function dateToYmd(date: Date): string {
		const year = date.getFullYear();
		const month = ('0' + (date.getMonth() + 1)).slice(-2);
		const day = ('0' + date.getDate()).slice(-2);
		return `${year}-${month}-${day}`;
	}

	function ymdToDate(dateString: string): Date {
		const [year, month, day] = dateString.split('-').map(Number);
		return new Date(year, month - 1, day);
	}

	function selectDay(day: string) {
		return () => {
			console.log('select day', day);
			currentDay = day;
			currentPlayerValue = 0;
			stop();
		};
	}

	type EditFrame = {
		sessionId: string;
		timestampMs: number;
		filepath: string;
		doc: string;
		ops: Delta[];
		delta: Delta;
	};

	let currentEdit: EditFrame | null = null;
	$: if (currentPlayerValue > 0) {
		let totalEdits = 0;
		let priorDeltas: Delta[] = [];
		currentEdit = null;
		dayPlaylist[currentDay].chapters.forEach((chapter) => {
			console.log(totalEdits);
			if (currentEdit == null && currentPlayerValue < totalEdits + chapter.editCount) {
				let thisEdit = chapter.edits[currentPlayerValue - totalEdits];
				priorDeltas = priorDeltas.concat(
					chapter.edits
						.slice(0, currentPlayerValue - totalEdits)
						.filter((edit) => edit.filepath == thisEdit?.filepath)
						.map((edit) => edit.delta)
				);

				console.log('prior', priorDeltas);
				console.log('current', thisEdit);

				currentEdit = {
					sessionId: chapter.session,
					timestampMs: thisEdit.delta.timestampMs,
					filepath: thisEdit.filepath,
					doc: sessionFiles[chapter.session][thisEdit.filepath],
					ops: priorDeltas.concat(thisEdit.delta),
					delta: thisEdit.delta
				};
			}
			totalEdits += chapter.editCount;
		});
	}

	// player
	let interval: ReturnType<typeof setInterval> | undefined;
	let direction: -1 | 1 = 1;
	let speed = 1;
	let oneSecond = 1000;

	const stop = () => {
		clearInterval(interval);
		interval = undefined;
		speed = 1;
	};
	const play = () => start({ direction, speed });

	const start = (params: { direction: 1 | -1; speed: number }) => {
		if (interval) clearInterval(interval);
		interval = setInterval(() => {
			currentPlayerValue += 1;
		}, oneSecond / params.speed);
	};

	const speedUp = () => {
		speed = speed * 2;
		start({ direction, speed });
	};
</script>

{#if $sessions.length === 0}
	<div class="flex h-full items-center justify-center">
		<div class="text-center">
			<h2 class="text-xl">I haven't seen any changes yet</h2>
			<p class="text-gray-500">Go code something!</p>
		</div>
	</div>
{:else}
	<div class="flex flex-row h-full w-full border">
		<div class="w-64 flex-shrink-0 border">
			<div>Playlist</div>
			{#each Object.entries(sessionDays) as [day, sessions]}
				<div class="flex flex-col border" on:click={selectDay(day)}>
					<div class="text-gray-500">{day}</div>
					{sessions.length}
				</div>
			{/each}
		</div>
		<div class="flex-grow">
			<div class="flex flex-col h-full w-full border">
				<div class="flex-grow border overflow-auto">
					{ymdToDate(currentDay)}
					{#if dayPlaylist[currentDay] !== undefined}
						{#if currentEdit !== null}
							<div class="h-full overflow-auto border">
								<CodeViewer
									doc={currentEdit.doc}
									deltas={currentEdit.ops}
									filepath={currentEdit.filepath}
								/>
							</div>
						{/if}
					{:else}
						loading...
					{/if}
				</div>
				<div class="border">
					{#if dayPlaylist[currentDay] !== undefined}
						<div>{dayPlaylist[currentDay].chapters.length} chapters</div>
						<div>{dayPlaylist[currentDay].editCount} edits</div>
						<div>{Math.round(dayPlaylist[currentDay].totalDurationMs / 1000 / 60)} min</div>
						{#if currentEdit !== null}
							<div>{currentEdit.sessionId}</div>
							<div>{currentEdit.filepath}</div>
							<div>{currentEdit.delta.timestampMs}</div>
							<div>{new Date(currentEdit.delta.timestampMs)}</div>
						{/if}
					{/if}
				</div>
				<div class="flex flex-row border">
					<div
						on:click={() => {
							currentPlayerValue += 1;
						}}
					>
						forward
					</div>
					<div class="w-full">
						{#if dayPlaylist[currentDay] !== undefined}
							<input
								type="range"
								class="w-full bg-white"
								max={dayPlaylist[currentDay].editCount}
								step="1"
								bind:value={currentPlayerValue}
							/>
						{/if}
					</div>
					<div class="mx-auto flex items-center gap-2">
						{#if interval}
							<button on:click={stop}><IconPlayerPauseFilled class="h-6 w-6" /></button>
						{:else}
							<button on:click={play}><IconPlayerPlayFilled class="h-6 w-6" /></button>
						{/if}
						<button on:click={speedUp}>{speed}x</button>
					</div>
				</div>
			</div>
		</div>
		<div class="w-64 flex-shrink-0 border overflow-auto">
			<div>Sessions</div>
			<div class="flex flex-col">
				{#each sessionDays[currentDay] as session}
					<div class="border overflow-auto">
						{new Date(session.meta.startTimestampMs).toLocaleTimeString()}
						to
						{new Date(session.meta.lastTimestampMs).toLocaleTimeString()}
						{#if sessionChapters[session.id] !== undefined}
							<div>{Math.round(sessionChapters[session.id].totalDurationMs / 1000 / 60)} min</div>
							{#each Object.entries(sessionChapters[session.id].files) as [filepath, count]}
								<div>{filepath}</div>
								<div>{count}</div>
							{/each}
						{/if}
					</div>
				{/each}
			</div>
		</div>
	</div>
{/if}
