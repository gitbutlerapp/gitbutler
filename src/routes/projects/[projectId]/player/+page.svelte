<script lang="ts">
	import type { PageData } from './$types';
	import { listFiles } from '$lib/sessions';
	import { type Delta, list as listDeltas } from '$lib/deltas';
	import { CodeViewer } from '$lib/components';
	import { IconPlayerPauseFilled, IconPlayerPlayFilled } from '$lib/components/icons';
	import { shortPath } from '$lib/paths';

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
		editOffsets: Record<string, number>;
		totalDurationMs: number;
		firstDeltaTimestampMs: number;
		lastDeltaTimestampMs: number;
	};

	let dayPlaylist: Record<string, DayVideo> = {};
	let sessionFiles: Record<string, Record<string, string>> = {};
	let sessionChapters: Record<string, VideoChapter> = {};

	$: currentSessions.forEach((s) => listSession(s.id));

	function listSession(sid: string) {
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

			// get the session chapters that are in the current day
			let dayChapters = Object.entries(sessionChapters)
				.filter(([sid, chapter]) => {
					let chapterDay = dateToYmd(new Date(chapter.firstDeltaTimestampMs));
					return chapterDay === currentDay;
				})
				.map(([, chapter]) => chapter)
				.sort((a, b) => a.firstDeltaTimestampMs - b.firstDeltaTimestampMs);

			let editCount = dayChapters.reduce((acc, chapter) => acc + chapter.editCount, 0);
			// for each entry in the day, reduce dayChapters to the number of edits up until that point
			let offsets: Record<string, number> = {};
			dayChapters.forEach((chapter, i) => {
				offsets[chapter.session] = dayChapters
					.slice(0, i)
					.reduce((acc, chapter) => acc + chapter.editCount, 0);
			});
			console.log(offsets);
			dayPlaylist[currentDay] = {
				chapters: dayChapters,
				editCount: editCount,
				editOffsets: offsets,
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
		});

		listFiles({
			projectId: data.projectId,
			sessionId: sid
		}).then((files) => {
			Object.entries(sessionFiles[sid]).forEach(([filepath, _]) => {
				if (files[filepath] !== undefined) {
					sessionFiles[sid][filepath] = files[filepath];
				}
			});
			setTimeout(() => {
				currentPlayerValue = 1;
			}, 1000);
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

	function ymdToDateLocale(dateString: string): string {
		let date = ymdToDate(dateString);
		return date.toLocaleString('en-US', { weekday: 'short', month: 'short', day: 'numeric' });
	}

	function dateRange(meta) {
		let day = new Date(meta.startTimestampMs).toLocaleString('en-US', {
			month: 'short',
			day: 'numeric'
		});
		let start = new Date(meta.startTimestampMs).toLocaleString('en-US', {
			hour: 'numeric',
			minute: 'numeric'
		});
		let end = new Date(meta.lastTimestampMs).toLocaleString('en-US', {
			hour: 'numeric',
			minute: 'numeric'
		});
		return `${day} ${start} - ${end}`;
	}

	function ymdToDay(dateString: string): number {
		let date = ymdToDate(dateString);
		return date.getDate();
	}

	const month = [
		'Jan',
		'Feb',
		'Mar',
		'Apr',
		'May',
		'Jun',
		'Jul',
		'Aug',
		'Sep',
		'Oct',
		'Nov',
		'Dec'
	];
	function ymdToMonth(dateString: string): string {
		let date = ymdToDate(dateString);
		return month[date.getMonth()];
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
			if (currentEdit == null && currentPlayerValue < totalEdits + chapter.editCount) {
				let thisEdit = chapter.edits[currentPlayerValue - totalEdits];
				priorDeltas = priorDeltas.concat(
					chapter.edits
						.slice(0, currentPlayerValue - totalEdits)
						.filter((edit) => edit.filepath == thisEdit?.filepath)
						.map((edit) => edit.delta)
				);

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
		scrollToSession();
	}

	function scrollToSession() {
		let sessionEl = document.getElementById('currentSession');
		if (sessionEl) {
			sessionEl.scrollIntoView({ behavior: 'smooth', block: 'center' });
		}
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
	<div class="flex h-full w-full flex-row bg-black">
		<div class="w-24 flex-shrink-0 border-r border-zinc-700 p-2">
			<div class="font-zinc-100 mb-2 text-lg font-bold">Daily Work</div>
			{#each Object.entries(sessionDays) as [day, sessions]}
				{#if day == currentDay}
					<div
						class="mb-2 flex cursor-pointer flex-col rounded bg-zinc-800 p-2 text-center text-white shadow"
						on:click={selectDay(day)}
					>
						<div class="">{ymdToDay(day)}</div>
						<div class="">{ymdToMonth(day)}</div>
					</div>
				{:else}
					<div
						class="mb-2 flex cursor-pointer flex-col rounded bg-zinc-900 p-2 text-center shadow"
						on:click={selectDay(day)}
					>
						<div class="">{ymdToDay(day)}</div>
						<div class="">{ymdToMonth(day)}</div>
					</div>
				{/if}
			{/each}
		</div>
		<div class="flex-grow">
			<div class="flex h-full w-full flex-col">
				<div class="flex-auto overflow-x-hidden overflow-y-scroll text-clip p-2">
					{#if dayPlaylist[currentDay] !== undefined}
						{#if currentEdit !== null}
							<CodeViewer
								doc={currentEdit.doc}
								deltas={currentEdit.ops}
								filepath={currentEdit.filepath}
							/>
						{/if}
					{:else}
						loading...
					{/if}
				</div>
				<div class="p-2">
					{#if dayPlaylist[currentDay] !== undefined}
						<div class="flex flex-row justify-between">
							<div>{dayPlaylist[currentDay].chapters.length} sessions</div>
							<div>{dayPlaylist[currentDay].editCount} edits</div>
							<div>{Math.round(dayPlaylist[currentDay].totalDurationMs / 1000 / 60)} min</div>
						</div>
						{#if currentEdit !== null}
							<div class="flex flex-row justify-between">
								<div class="font-mono font-bold text-white">{currentEdit.filepath}</div>
								<div>{new Date(currentEdit.delta.timestampMs).toLocaleString('en-US')}</div>
							</div>
						{/if}
					{/if}
				</div>
				<div class="flex flex-col bg-zinc-800 p-2 p-2">
					{#if dayPlaylist[currentDay] !== undefined}
						<div class="h-0 w-full justify-between">
							{#each dayPlaylist[currentDay].chapters as chapter}
								<div
									class="inline-block h-2 rounded bg-white"
									style="width: {Math.round(
										(chapter.editCount / dayPlaylist[currentDay].editCount) * 100
									)}%"
								>
									&nbsp;
								</div>
							{/each}
						</div>
					{/if}
					<div class="w-full">
						{#if dayPlaylist[currentDay] !== undefined}
							<input
								type="range"
								class="-mt-3 w-full cursor-pointer appearance-none rounded-lg border-transparent bg-transparent"
								max={dayPlaylist[currentDay].editCount}
								step="1"
								bind:value={currentPlayerValue}
							/>
						{/if}
					</div>
					<div class="mx-auto flex items-center gap-2">
						<button
							on:click={() => {
								currentPlayerValue -= 1;
							}}
						>
							<svg
								xmlns="http://www.w3.org/2000/svg"
								fill="none"
								viewBox="0 0 24 24"
								stroke-width="1.5"
								stroke="currentColor"
								class="icon-pointer h-6 w-6"
							>
								<path
									stroke-linecap="round"
									stroke-linejoin="round"
									d="M21 16.811c0 .864-.933 1.405-1.683.977l-7.108-4.062a1.125 1.125 0 010-1.953l7.108-4.062A1.125 1.125 0 0121 8.688v8.123zM11.25 16.811c0 .864-.933 1.405-1.683.977l-7.108-4.062a1.125 1.125 0 010-1.953L9.567 7.71a1.125 1.125 0 011.683.977v8.123z"
								/>
							</svg>
						</button>
						<button
							on:click={() => {
								currentPlayerValue += 1;
							}}
						>
							<svg
								xmlns="http://www.w3.org/2000/svg"
								fill="none"
								viewBox="0 0 24 24"
								stroke-width="1.5"
								stroke="currentColor"
								class="icon-pointer h-6 w-6"
							>
								<path
									stroke-linecap="round"
									stroke-linejoin="round"
									d="M3 8.688c0-.864.933-1.405 1.683-.977l7.108 4.062a1.125 1.125 0 010 1.953l-7.108 4.062A1.125 1.125 0 013 16.81V8.688zM12.75 8.688c0-.864.933-1.405 1.683-.977l7.108 4.062a1.125 1.125 0 010 1.953l-7.108 4.062a1.125 1.125 0 01-1.683-.977V8.688z"
								/>
							</svg>
						</button>
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
		<div class="w-80 flex-shrink-0 overflow-auto border-l border-zinc-700 bg-black p-2">
			<div class="flex flex-row justify-between">
				<div class="font-zinc-100 mb-2 text-lg font-bold">Sessions</div>
				<div>{Object.entries(sessionDays[currentDay]).length}</div>
			</div>
			<div class="flex flex-col">
				{#each sessionDays[currentDay] as session}
					{#if currentEdit !== null && currentEdit.sessionId == session.id}
						<div
							id="currentSession"
							class="mb-2 overflow-auto rounded border-zinc-800 bg-zinc-700 text-white shadow"
						>
							<div class="flex flex-row justify-between px-2 pt-2">
								<div class="font-bold">{dateRange(session.meta)}</div>
								{#if sessionChapters[session.id] !== undefined}
									<div>
										{Math.round(sessionChapters[session.id].totalDurationMs / 1000 / 60)} min
									</div>
								{/if}
							</div>
							<div class="flex flex-row justify-between px-2 pb-1">
								{#if sessionChapters[session.id] !== undefined}
									<div>{Object.entries(sessionChapters[session.id].files).length} files</div>
								{/if}
							</div>
							{#if sessionChapters[session.id] !== undefined}
								<div class="bg-zinc-800 p-2">
									{#each Object.entries(sessionChapters[session.id].files) as [filenm, changes]}
										<div>{shortPath(filenm)}</div>
									{/each}
								</div>
							{/if}
						</div>
					{:else}
						<div
							on:click={() => {
								currentPlayerValue = max(dayPlaylist[currentDay].editOffsets[session.id], 1);
							}}
							class="pointer-cursor mb-2 overflow-auto rounded border-zinc-800 bg-zinc-800 shadow"
						>
							<div class="flex flex-row justify-between px-2 pt-2">
								<div>{dateRange(session.meta)}</div>
								{#if sessionChapters[session.id] !== undefined}
									<div>
										{Math.round(sessionChapters[session.id].totalDurationMs / 1000 / 60)} min
									</div>
								{/if}
							</div>
							<div class="flex flex-row justify-between px-2 pb-2 text-zinc-400">
								{#if sessionChapters[session.id] !== undefined}
									<div>{Object.entries(sessionChapters[session.id].files).length} files</div>
								{/if}
							</div>
						</div>
					{/if}
				{/each}
			</div>
		</div>
	</div>
{/if}
