<script lang="ts">
	import type { PageData } from './$types';
	import { listFiles } from '$lib/sessions';
	import { type Delta, list as listDeltas } from '$lib/deltas';
	import { CodeViewer } from '$lib/components';
	import { IconPlayerPauseFilled, IconPlayerPlayFilled } from '$lib/components/icons';
	import { shortPath } from '$lib/paths';
	import { format } from 'date-fns';
	import type { Session } from '$lib/sessions';

	export let data: PageData;

	const { sessions } = data;

	let currentPlayerValue = 0;
	let currentDay = dateToYmd(new Date());

	const urlParams = new URLSearchParams(window.location.search);
	let fileFilter = urlParams.get('file');

	$: sessionDays = $sessions.reduce((group: Record<string, Session[]>, session) => {
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

	$: currentSessions.forEach((s) => processSession(s.id));

	function processSession(sid: string) {
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
				if (fileFilter && !filepath.includes(fileFilter)) return;

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
				sessionChapters[sid].firstDeltaTimestampMs = Math.min(
					deltas.at(0)!.timestampMs,
					sessionChapters[sid].firstDeltaTimestampMs
				);
				sessionChapters[sid].lastDeltaTimestampMs = Math.max(
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
				// filter to chapters with more than 0 files
				.filter(([, chapter]) => Object.keys(chapter.files).length > 0)
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
			console.log(dayChapters);
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
					(acc, chapter) => Math.min(acc, chapter.firstDeltaTimestampMs),
					9999999999999
				),
				lastDeltaTimestampMs: Object.values(dayChapters).reduce(
					(acc, chapter) => Math.max(acc, chapter.lastDeltaTimestampMs),
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

	function dateToYmd(date: Date): string {
		return format(date, 'yyyy-MM-dd');
	}

	function ymdToDate(dateString: string): Date {
		return new Date(dateString);
	}

	function dateRange(chapter: VideoChapter) {
		let day = new Date(chapter.firstDeltaTimestampMs).toLocaleString('en-US', {
			month: 'short',
			day: 'numeric'
		});
		let start = new Date(chapter.firstDeltaTimestampMs).toLocaleString('en-US', {
			hour: 'numeric',
			minute: 'numeric'
		});
		let end = new Date(chapter.lastDeltaTimestampMs).toLocaleString('en-US', {
			hour: 'numeric',
			minute: 'numeric'
		});
		return `${day} ${start} - ${end}`;
	}

	function ymdToDay(dateString: string): number {
		let date = ymdToDate(dateString);
		return date.getDate();
	}

	function ymdToMonth(dateString: string): string {
		return format(new Date(dateString), 'MMM');
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
		const sessionEl = document.getElementById('currentSession');
		if (sessionEl) {
			sessionEl.scrollIntoView({ behavior: 'smooth', block: 'center' });
		}

		const changedLines = document.getElementsByClassName('line-changed');
		if (changedLines.length > 0) {
			changedLines[0].scrollIntoView({ behavior: 'smooth', block: 'center' });
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

	// <!-- svelte-ignore a11y-click-events-have-key-events -->
	function handleKey() {}
</script>

{#if $sessions.length === 0}
	<div class="flex h-full items-center justify-center">
		<div class="text-center">
			<h2 class="text-xl">I haven't seen any changes yet</h2>
			<p class="text-gray-500">Go code something!</p>
		</div>
	</div>
{:else}
	<div id="player-page" class="flex h-full w-full">
		<div class="flex flex-col h-full w-full">
			{#if fileFilter}
				<div class="w-full p-2 font-mono text-lg">{fileFilter}</div>
			{/if}
			<div class="flex flex-row h-full w-full">
				<div id="left" class="flex h-full w-20 flex-shrink-0 flex-col p-2">
					<div class="overflow-y-auto">
						{#each Object.entries(sessionDays) as [day, sessions]}
							{#if day == currentDay}
								<div
									class="mb-2 flex cursor-pointer flex-col rounded bg-zinc-600 border border-zinc-500 p-2 text-center text-white shadow"
									on:keydown={handleKey}
									on:click={selectDay(day)}
								>
									<div class="font-bold text-lg">{ymdToDay(day)}</div>
									<div class="">{ymdToMonth(day)}</div>
								</div>
							{:else}
								<div
									class="mb-2 flex cursor-pointer flex-col rounded bg-zinc-700 border border-zinc-600 p-2 text-center shadow"
									on:keydown={handleKey}
									on:click={selectDay(day)}
								>
									<div class="font-bold text-lg">{ymdToDay(day)}</div>
									<div class="">{ymdToMonth(day)}</div>
								</div>
							{/if}
						{/each}
					</div>
				</div>

				<div id="right" class="w-80 xl:w-96 flex-shrink-0 overflow-auto p-2">
					<div class="border border-zinc-600 bg-zinc-800 rounded-t h-full">
						<div class="flex flex-row justify-between bg-zinc-600">
							<div class="font-zinc-100 text-lg p-2">Activities</div>
							{#if dayPlaylist[currentDay] !== undefined}
								<div class="p-2 text-zinc-400">{dayPlaylist[currentDay].chapters.length}</div>
							{/if}
						</div>
						{#if dayPlaylist[currentDay] !== undefined}
							<div class="flex flex-col bg-zinc-700 p-2">
								{#each dayPlaylist[currentDay].chapters as chapter}
									{#if currentEdit !== null && currentEdit.sessionId == chapter.session}
										<div
											id="currentSession"
											class="mb-2 overflow-auto rounded border border-zinc-500 bg-zinc-600 text-white shadow"
										>
											<div class="flex flex-row justify-between px-2 pt-2">
												<div class="">{dateRange(chapter)}</div>
												<div>
													{Math.round(chapter.totalDurationMs / 1000 / 60)} min
												</div>
											</div>
											{#if chapter.files}
												<div class="flex flex-row justify-between px-2 pb-2">
													<div>{Object.entries(chapter.files).length} files</div>
												</div>
												<div class="bg-zinc-800 p-2">
													{#each Object.entries(chapter.files) as [filenm, changes]}
														<div class="text-zinc-500">{shortPath(filenm)}</div>
													{/each}
												</div>
											{/if}
										</div>
									{:else}
										<div
											on:keydown={handleKey}
											on:click={() => {
												currentPlayerValue = Math.max(
													dayPlaylist[currentDay].editOffsets[chapter.session],
													1
												);
											}}
											class="cursor-pointer mb-2 overflow-auto rounded border border-zinc-500 bg-zinc-600 shadow"
										>
											<div class="flex flex-row justify-between px-2 pt-2">
												<div class="font-zinc-600">{dateRange(chapter)}</div>
												<div>
													{Math.round(chapter.totalDurationMs / 1000 / 60)} min
												</div>
											</div>
											<div class="flex flex-row justify-between px-2 pb-2 text-zinc-400">
												<div>{Object.entries(chapter.files).length} files</div>
											</div>
										</div>
									{/if}
								{/each}
							</div>
						{/if}
					</div>
				</div>

				<div id="middle" class="flex-auto overflow-auto">
					<div class="flex h-full w-full flex-col gap-2">
						<div id="code" class="flex-auto overflow-auto px-2">
							{#if dayPlaylist[currentDay] !== undefined}
								{#if currentEdit !== null}
									<CodeViewer
										doc={currentEdit.doc}
										deltas={currentEdit.ops}
										filepath={currentEdit.filepath}
									/>
								{/if}
							{:else}
								<span class="m-auto">loading...</span>
							{/if}
						</div>

						<div id="info" class="px-2">
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

						<div id="controls" class="flex flex-col bg-zinc-800 px-2">
							{#if dayPlaylist[currentDay] !== undefined}
								<div class="flex h-0 w-full justify-between">
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
			</div>
		</div>
	</div>
{/if}
