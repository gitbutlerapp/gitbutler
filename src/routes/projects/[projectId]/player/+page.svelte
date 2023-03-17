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

	let currentPlayerValue: number | null = null;
	let showLatest = false;
	let fullContext = false;
	let context = 8;

	const urlParams = new URLSearchParams(window.location.search);
	$: currentDay = urlParams.get('date') ?? Object.keys(sessionDays)[0] ?? '';

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

			// process the playlist metadata
			dayPlaylist[currentDay] = processDayPlaylist(dayChapters);
		});

		listFiles({
			projectId: data.projectId,
			sessionId: sid
		}).then((files) => {
			Object.entries(sessionFiles[sid] ?? {}).forEach(([filepath, _]) => {
				if (files[filepath] !== undefined) {
					sessionFiles[sid][filepath] = files[filepath];
				}
			});
			setTimeout(() => {
				currentPlayerValue = 0;
			}, 1000);
		});
	}

	function processDayPlaylist(dayChapters: VideoChapter[]): DayVideo {
		let editCount = dayChapters.reduce((acc, chapter) => acc + chapter.editCount, 0);
		// for each entry in the day, reduce dayChapters to the number of edits up until that point
		let offsets: Record<string, number> = {};
		dayChapters.forEach((chapter, i) => {
			offsets[chapter.session] = dayChapters
				.slice(0, i)
				.reduce((acc, chapter) => acc + chapter.editCount, 0);
		});
		return {
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
			showLatest = false;
			currentPlayerValue = 0;
			stop();
		};
	}

	function selectLatest() {
		return () => {
			console.log('select latest', Object.keys(sessionDays)[0]);
			showLatest = true;
			currentDay = Object.keys(sessionDays)[0]; // get latest day
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

	let currentPlaylist: DayVideo | null = null;
	let currentEdit: EditFrame | null = null;

	$: if (currentDay && dayPlaylist && dayPlaylist[currentDay]) {
		currentPlaylist = dayPlaylist[currentDay];
		if (currentPlaylist !== null) {
			if (showLatest) {
				// make currentPlaylist.chapters just the last chapter
				let latestChapter = currentPlaylist.chapters[currentPlaylist.chapters.length - 1];
				let playlist: VideoChapter[] = [];
				playlist.push(latestChapter);

				if (latestChapter.edits.length < 20) {
					// if there are less than 20 edits, get the previous chapter
					latestChapter = currentPlaylist.chapters[currentPlaylist.chapters.length - 2];
					if (latestChapter !== undefined) {
						playlist.push(latestChapter);
						playlist.reverse();
					}
				}

				currentPlaylist = processDayPlaylist(playlist);
			}
		}
	}

	$: if (currentPlaylist !== null) {
		let totalEdits = 0;
		let priorDeltas: Delta[] = [];
		currentEdit = null;
		currentPlaylist?.chapters.forEach((chapter) => {
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
			incrementPlayerValue();
		}, oneSecond / params.speed);
	};

	const changePlayerValue = (amount: number) => {
		if (currentPlayerValue !== null) {
			currentPlayerValue += amount;
			if (currentPlayerValue < 0) {
				currentPlayerValue = 0;
			} else if (currentPlaylist && currentPlayerValue >= currentPlaylist.editCount) {
				currentPlayerValue = currentPlaylist.editCount - 1;
			}
		} else {
			currentPlayerValue = 0;
		}
	};

	const incrementPlayerValue = () => {
		changePlayerValue(1);
	};

	const decrementPlayerValue = () => {
		changePlayerValue(-1);
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
{:else if currentPlaylist !== null}
	<div id="player-page" class="flex h-full w-full">
		<div class="flex h-full w-full flex-col">
			{#if fileFilter}
				<div class="w-full p-2 font-mono text-lg" on:click={() => (fileFilter = null)}>
					{fileFilter}
				</div>
			{/if}
			<div class="flex h-full w-full flex-row">
				<div id="left" class="day-of-week flex h-full flex-shrink-0 flex-col p-2 pb-1">
					<div class="overflow-y-auto">
						<div
							class="card-latest mb-2 flex cursor-pointer flex-col rounded border text-zinc-300 border-t-[0.5px] border-r-[0.5px] border-b-[0.5px] border-l-[0.5px] border-gb-700 {showLatest
								? 'border-gb-700 bg-gb-800 text-white'
								: 'border-gb-700 bg-gb-900'} p-2 text-center shadow"
							on:keydown={handleKey}
							on:click={selectLatest()}
						>
							<div class="">Latest</div>
						</div>
						{#each Object.entries(sessionDays) as [day, sessions]}
							<div
								class="card-day-of-week mb-2 text-zinc-300 border-t-[0.5px] border-r-[0.5px] border-b-[0.5px] border-l-[0.5px] border-gb-700 {day ==
									currentDay && !showLatest
									? 'border-gb-700 bg-gb-800 text-white'
									: 'border-gb-700 bg-gb-900'} flex cursor-pointer flex-col rounded border p-2 pb-1 text-center shadow transition duration-150 ease-out hover:ease-in hover:bg-gb-800"
								on:keydown={handleKey}
								on:click={selectDay(day)}
							>
								<div class="text-xl leading-5">{ymdToDay(day)}</div>
								<div class="leading-4">{ymdToMonth(day)}</div>
							</div>
						{/each}
					</div>
				</div>

				<div id="right" class="h-full w-80 flex-shrink-0 p-2 xl:w-96">
					<div
						class="flex h-full flex-col rounded border-t-[0.5px] border-r-[0.5px] border-b-[0.5px] border-l-[0.5px] border-gb-700 bg-gb-900"
					>
						<div
							class="card-header flex flex-row justify-between rounded-t border-b-[1px] border-b-gb-750 bg-gb-800"
						>
							<div class="p-3 text-lg text-zinc-300">
								<div class="flex flex-row items-center space-x-2">
									<div>Activities</div>
									<div class="text-sm text-zinc-400">
										{currentPlaylist.chapters.length}
									</div>
								</div>
							</div>
						</div>

						<div
							class="card-list flex h-full flex-col space-y-2 overflow-auto rounded-b bg-gb-900 p-2 "
						>
							{#if currentPlaylist.chapters.length == 0}
								<div class="text-center text-zinc-300 mt-4">No activities found</div>
							{/if}
							{#each currentPlaylist.chapters as chapter}
								{#if currentEdit !== null && currentEdit.sessionId == chapter.session}
									<div
										id="currentSession"
										class="session-card rounded border-[0.5px] border-gb-700 text-zinc-300 shadow-md"
									>
										<div class="flex flex-row justify-between rounded-t bg-gb-800 px-3 pt-3">
											<div class="">{dateRange(chapter)}</div>
											<div>
												{Math.round(chapter.totalDurationMs / 1000 / 60)} min
											</div>
										</div>
										{#if chapter.files}
											<div class=" flex flex-row justify-between bg-gb-800 px-3 pb-3">
												<div>
													{Object.entries(chapter.files).length}
													{Object.entries(chapter.files).length > 1 ? 'files' : 'file'}
												</div>
											</div>
											<div class="rounded-b bg-zinc-800 p-2 pb-3">
												{#each Object.entries(chapter.files) as [filenm, changes]}
													{#if currentEdit.filepath == filenm}
														<div class="text-zinc-100 font-bold truncate">{shortPath(filenm)}</div>
													{:else}
														<div class="text-zinc-500 truncate">{shortPath(filenm)}</div>
													{/if}
												{/each}
											</div>
										{/if}
									</div>
								{:else}
									<div
										on:keydown={handleKey}
										on:click={() => {
											currentPlayerValue = Math.max(
												currentPlaylist.editOffsets[chapter.session],
												1
											);
										}}
										class="session-card cursor-pointer rounded border-[0.5px] border-gb-700 bg-gb-900 shadow-md hover:bg-gb-800 transition duration-150 ease-out hover:ease-in"
									>
										<div class="flex flex-row justify-between px-3 pt-3">
											<div class="font-zinc-600">{dateRange(chapter)}</div>
											<div>
												{Math.round(chapter.totalDurationMs / 1000 / 60)} min
											</div>
										</div>
										<div class="flex flex-row justify-between px-3 pb-3 text-zinc-400">
											<div>
												{Object.entries(chapter.files).length}
												{Object.entries(chapter.files).length > 1 ? 'files' : 'file'}
											</div>
										</div>
									</div>
								{/if}
							{/each}
						</div>
					</div>
				</div>

				<div
					id="middle"
					class="m-2 flex-auto overflow-auto rounded border border-zinc-700 bg-gb-900 "
				>
					<div class="relative flex h-full w-full flex-col gap-2 ">
						<div id="code" class="h-full w-full flex-auto overflow-auto px-2 pb-[120px]">
							{#if dayPlaylist[currentDay] !== undefined}
								{#if currentEdit !== null}
									<CodeViewer
										doc={currentEdit.doc}
										deltas={currentEdit.ops}
										filepath={currentEdit.filepath}
										context={fullContext ? 100000 : context}
									/>
								{:else}
									<div class="mt-8 text-center">Select a playlist</div>
								{/if}
							{:else}
								<span class="m-auto">loading...</span>
							{/if}
						</div>

						{#if currentEdit !== null}
							<div id="info" class="absolute bottom-[64px] left-4 rounded-lg bg-zinc-800 p-2">
								<div class="flex flex-row justify-between space-x-2">
									<div class="font-mono font-bold text-white">{currentEdit.filepath}</div>
									<div>{new Date(currentEdit.delta.timestampMs).toLocaleString('en-US')}</div>
								</div>
							</div>

							<div
								id="controls"
								class="absolute bottom-0 flex w-full flex-col border-t border-zinc-700 bg-[#2E2E32]/75 p-2"
								style="
								border-width: 0.5px; 
								-webkit-backdrop-filter: blur(20px) saturate(190%) contrast(70%) brightness(80%);
								backdrop-filter: blur(20px) saturate(190%) contrast(70%) brightness(80%);
								background-color: rgba(24, 24, 27, 0.60);
								border: 0.5px solid rgba(63, 63, 70, 0.50);
							"
							>
								<div class="flex h-0 w-full justify-between">
									{#each currentPlaylist.chapters as chapter}
										<div
											class="inline-block h-2 rounded bg-white"
											style="width: {Math.round(
												(chapter.editCount / currentPlaylist.editCount) * 100
											)}%"
										>
											&nbsp;
										</div>
									{/each}
								</div>
								<div class="w-full">
									<input
										type="range"
										class="-mt-3 w-full cursor-pointer appearance-none rounded-lg border-transparent bg-transparent"
										max={currentPlaylist.editCount - 1}
										step="1"
										bind:value={currentPlayerValue}
									/>
								</div>
								<div class="mx-auto flex items-center gap-2">
									<button on:click={decrementPlayerValue}>
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
									<button on:click={incrementPlayerValue}>
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
									<div>
										<input type="checkbox" bind:checked={fullContext} /> Full Context
										{#if !fullContext}
											<input type="number" bind:value={context} />
										{/if}
									</div>
								</div>
							</div>
						{/if}
					</div>
				</div>
			</div>
		</div>
	</div>
{:else}
	<div class="p-20 text-center">loading data...</div>
{/if}
