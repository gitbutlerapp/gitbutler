<script lang="ts">
	import type { PageData } from './$types';
	import { listFiles } from '$lib/sessions';
	import { type Delta, list as listDeltas } from '$lib/deltas';
	import { CodeViewer } from '$lib/components';
	import { IconPlayerPauseFilled, IconPlayerPlayFilled } from '$lib/components/icons';
	import { shortPath } from '$lib/paths';
	import { format } from 'date-fns';
	import type { Session } from '$lib/sessions';
	import { page } from '$app/stores';
	import { goto } from '$app/navigation';
	import { derived, writable } from 'svelte/store';

	export let data: PageData;

	const { sessions } = data;

	const currentDeltaIndex = writable(parseInt($page.url.searchParams.get('delta') ?? '0'));
	currentDeltaIndex.subscribe((index) => {
		$page.url.searchParams.set('delta', index.toString());
		goto($page.url.href);
	});

	let showLatest = false;
	let fullContext = false;
	let context = 8;

	const sessionDays = derived(sessions, (sessions) =>
		sessions.reduce((group: Record<string, Session[]>, session) => {
			const day = dateToYmd(new Date(session.meta.startTimestampMs));
			group[day] = group[day] ?? [];
			group[day].push(session);
			// sort by startTimestampMs
			group[day].sort((a, b) => a.meta.startTimestampMs - b.meta.startTimestampMs);
			return group;
		}, {})
	);

	const currentDay = derived(
		[page, sessionDays],
		([page, sessionDays]) => page.url.searchParams.get('date') ?? Object.keys(sessionDays)[0] ?? ''
	);

	const fileFilter = writable($page.url.searchParams.get('file'));
	fileFilter.subscribe((filter) => {
		if (filter) {
			$page.url.searchParams.set('file', filter);
		} else {
			$page.url.searchParams.delete('file');
		}
		goto($page.url.href);
	});

	$: currentSessions = $sessions.filter((session) => {
		let sessionDay = dateToYmd(new Date(session.meta.startTimestampMs));
		return sessionDay === $currentDay;
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

	const processSession = (sid: string) => {
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
				if ($fileFilter && !filepath.includes($fileFilter)) return;

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
					deltas.at(0)?.timestampMs || 0,
					sessionChapters[sid].firstDeltaTimestampMs
				);
				sessionChapters[sid].lastDeltaTimestampMs = Math.max(
					deltas.at(-1)?.timestampMs || 0,
					sessionChapters[sid].lastDeltaTimestampMs
				);
				sessionChapters[sid].totalDurationMs =
					sessionChapters[sid].lastDeltaTimestampMs - sessionChapters[sid].firstDeltaTimestampMs;
			});

			// get the session chapters that are in the current day
			let dayChapters = Object.values(sessionChapters)
				.filter((chapter) => {
					let chapterDay = dateToYmd(new Date(chapter.firstDeltaTimestampMs));
					return chapterDay === $currentDay;
				})
				// filter to chapters with more than 0 files
				.filter((chapter) => Object.keys(chapter.files).length > 0)
				.map((chapter) => chapter)
				.sort((a, b) => a.firstDeltaTimestampMs - b.firstDeltaTimestampMs);

			// process the playlist metadata
			dayPlaylist[$currentDay] = processDayPlaylist(dayChapters);
		});

		listFiles({
			projectId: data.projectId,
			sessionId: sid
		}).then((files) => {
			Object.keys(sessionFiles[sid] ?? {}).forEach((filepath) => {
				if (files[filepath] !== undefined) {
					sessionFiles[sid][filepath] = files[filepath];
				}
			});
		});
	};

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

	const selectDay = (day: string, latest = false) => {
		showLatest = latest;
		$currentDeltaIndex = 0;
		stop();
		$page.url.searchParams.set('date', day);
		goto($page.url.href);
	};

	const selectLatestDay = () => {
		const latestDay = Object.keys($sessionDays)[0];
		selectDay(latestDay, true);
	};

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

	$: if ($currentDay && dayPlaylist && dayPlaylist[$currentDay]) {
		currentPlaylist = dayPlaylist[$currentDay];
		if (currentPlaylist !== null) {
			if (showLatest) {
				// make currentPlaylist.chapters just the last chapter
				let latestChapter = currentPlaylist.chapters[currentPlaylist.chapters.length - 1];
				let playlist: VideoChapter[] = [];
				playlist.push(latestChapter);

				if (latestChapter && latestChapter.edits.length < 20) {
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
			if (currentEdit == null && $currentDeltaIndex < totalEdits + chapter.editCount) {
				let thisEdit = chapter.edits[$currentDeltaIndex - totalEdits];
				priorDeltas = priorDeltas.concat(
					chapter.edits
						.slice(0, $currentDeltaIndex - totalEdits)
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
		const sessionEl = document.getElementById('current-session');
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
		if ($currentDeltaIndex !== null) {
			$currentDeltaIndex += amount;
			if ($currentDeltaIndex < 0) {
				$currentDeltaIndex = 0;
			} else if (currentPlaylist && $currentDeltaIndex >= currentPlaylist.editCount) {
				$currentDeltaIndex = currentPlaylist.editCount - 1;
			}
		} else {
			$currentDeltaIndex = 0;
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
			{#if $fileFilter}
				<button
					class="w-full p-2 text-left font-mono text-lg"
					on:click={() => ($fileFilter = null)}
				>
					{$fileFilter}
				</button>
			{/if}
			<div class="flex h-full w-full flex-row gap-2 p-2">
				<ul id="days" class="flex h-full flex-shrink-0 flex-col gap-2 overflow-y-scroll">
					<li class="w-full">
						<button
							class:bg-gb-800={showLatest}
							class:text-white={showLatest}
							class:border-gb-700={!showLatest}
							class:bg-gb-900={!showLatest}
							class="rounded border border-[0.5px] border-gb-700 border-gb-700 p-2 text-center text-zinc-300 shadow"
							on:click={() => selectLatestDay()}
						>
							Latest
						</button>
					</li>
					{#each Object.keys($sessionDays) as day}
						<li class="w-full">
							<button
								class:bg-gb-800={day === $currentDay && !showLatest}
								class:text-white={day === $currentDay && !showLatest}
								class:border-gb-700={day !== $currentDay || showLatest}
								class:bg-gb-900={day !== $currentDay || showLatest}
								class="flex w-full flex-col items-center rounded border border-[0.5px] border-gb-700 border-gb-700 p-2 text-zinc-300 shadow transition duration-150 ease-out hover:bg-gb-800 hover:ease-in"
								on:click={() => selectDay(day)}
							>
								<div class="text-xl leading-5">{ymdToDay(day)}</div>
								<div class="leading-4">{ymdToMonth(day)}</div>
							</button>
						</li>
					{/each}
				</ul>

				<article
					id="activities"
					class="flex h-full h-full w-80 flex-shrink-0 flex-col rounded border-[0.5px] border-gb-700 bg-gb-900 xl:w-96"
				>
					<header
						class="card-header flex flex-row justify-between rounded-t border-b-[1px] border-b-gb-750 bg-gb-800"
					>
						<h2 class="flex flex-row items-baseline space-x-2  p-3 text-lg text-zinc-300">
							<span>Activities</span>
							<span class="text-sm text-zinc-400">
								{currentPlaylist.chapters.length}
							</span>
						</h2>
					</header>

					<ul class="flex h-full flex-col gap-2 overflow-auto rounded-b bg-gb-900 p-2">
						{#each currentPlaylist.chapters as chapter}
							{@const isCurrent = currentEdit !== null && currentEdit.sessionId == chapter.session}
							<li
								id={isCurrent ? 'current-session' : ''}
								class="session-card rounded border-[0.5px] border-gb-700 text-zinc-300 shadow-md"
							>
								<button
									disabled={isCurrent}
									class="w-full"
									on:click={() => {
										$currentDeltaIndex = Math.max(
											currentPlaylist?.editOffsets[chapter.session] || 0,
											1
										);
									}}
								>
									<div class="flex flex-row justify-between rounded-t bg-gb-800 px-3 pt-3">
										<span>{dateRange(chapter)}</span>
										<span>
											{Math.round(chapter.totalDurationMs / 1000 / 60)} min
										</span>
									</div>

									<span class="flex flex-row justify-between bg-gb-800 px-3 pb-3">
										{Object.entries(chapter.files).length}
										{Object.entries(chapter.files).length > 1 ? 'files' : 'file'}
									</span>

									{#if isCurrent}
										<ul class="rounded-b bg-zinc-800 p-2">
											{#each Object.keys(chapter.files) as filename}
												<li
													class:text-zinc-100={currentEdit?.filepath == filename}
													class:font-bold={currentEdit?.filepath == filename}
													class="truncate text-left text-zinc-500"
												>
													{shortPath(filename)}
												</li>
											{/each}
										</ul>
									{/if}
								</button>
							</li>
						{:else}
							<div class="mt-4 text-center text-zinc-300">No activities found</div>
						{/each}
					</ul>
				</article>

				<div id="player" class="flex-auto overflow-auto rounded border border-zinc-700 bg-gb-900 ">
					<div class="relative flex h-full w-full flex-col gap-2 ">
						<div id="code" class="h-full w-full flex-auto overflow-auto px-2 pb-[120px]">
							{#if dayPlaylist[$currentDay] !== undefined}
								{#if currentEdit !== null}
									<CodeViewer
										doc={currentEdit.doc}
										deltas={currentEdit.ops}
										filepath={currentEdit.filepath}
										paddingLines={fullContext ? 100000 : context}
									/>
								{:else}
									<div class="mt-8 text-center">Select a playlist</div>
								{/if}
							{:else}
								<span class="m-auto">loading...</span>
							{/if}
						</div>

						{#if currentEdit !== null}
							<div id="info" class="absolute bottom-[86px] left-4 rounded-lg bg-zinc-800 p-2">
								<div class="flex flex-row justify-between space-x-2">
									<div class="font-mono font-bold text-white">{currentEdit.filepath}</div>
									<div>{new Date(currentEdit.delta.timestampMs).toLocaleString('en-US')}</div>
								</div>
							</div>

							<div
								id="controls"
								class="absolute bottom-0 flex w-full flex-col border-t border-zinc-700 bg-[#2E2E32]/75 p-2 pt-4"
								style="
								border-width: 0.5px; 
								-webkit-backdrop-filter: blur(5px) saturate(190%) contrast(70%) brightness(80%);
								backdrop-filter: blur(5px) saturate(190%) contrast(70%) brightness(80%);
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
										bind:value={$currentDeltaIndex}
									/>
								</div>

								<div
									class="playback-controller-ui mx-auto flex w-full items-center justify-between gap-2"
								>
									<div class="left-side flex space-x-8">
										<div class="play-button-button-container">
											{#if interval}
												<button on:click={stop}>
													<IconPlayerPauseFilled
														class="playback-button-play icon-pointer h-6 w-6"
													/>
												</button>
											{:else}
												<button on:click={play}>
													<IconPlayerPlayFilled class="icon-pointer h-6 w-6" />
												</button>
											{/if}
										</div>

										<div class="back-forward-button-container ">
											<button on:click={decrementPlayerValue} class="playback-button-back group">
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

											<button on:click={incrementPlayerValue} class="playback-button-forward group">
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

									<div class="align-center flex flex-row-reverse gap-2">
										<button class="checkbox-button ">
											<label
												for="full-context-checkbox"
												class="group block cursor-pointer rounded  transition-colors duration-200 ease-in-out hover:bg-zinc-700 "
											>
												<input
													type="checkbox"
													id="full-context-checkbox"
													bind:checked={fullContext}
													class="peer hidden"
												/>

												<svg
													fill="none"
													xmlns="http://www.w3.org/2000/svg"
													class="group h-8 w-8 rounded p-1.5 peer-checked:hidden"
												>
													<path
														d="M10.177 2.07944L13.073 5.21176C13.1081 5.24957 13.1319 5.2978 13.1416 5.35031C13.1513 5.40283 13.1464 5.45727 13.1274 5.50674C13.1084 5.55621 13.0763 5.59848 13.0351 5.62818C12.9939 5.65789 12.9455 5.67369 12.896 5.6736H10.75V7.0256C10.75 7.24074 10.671 7.44707 10.5303 7.5992C10.3897 7.75133 10.1989 7.8368 10 7.8368C9.80109 7.8368 9.61032 7.75133 9.46967 7.5992C9.32902 7.44707 9.25 7.24074 9.25 7.0256V5.6736H7.104C7.05449 5.67369 7.00607 5.65789 6.96487 5.62818C6.92368 5.59848 6.89157 5.55621 6.87261 5.50674C6.85365 5.45727 6.8487 5.40283 6.85838 5.35031C6.86806 5.2978 6.89195 5.24957 6.927 5.21176L9.823 2.07944C9.84622 2.05426 9.87381 2.03428 9.90418 2.02065C9.93456 2.00702 9.96712 2 10 2C10.0329 2 10.0654 2.00702 10.0958 2.02065C10.1262 2.03428 10.1538 2.05426 10.177 2.07944ZM9.25 12.9744C9.25 12.7593 9.32902 12.5529 9.46967 12.4008C9.61032 12.2487 9.80109 12.1632 10 12.1632C10.1989 12.1632 10.3897 12.2487 10.5303 12.4008C10.671 12.5529 10.75 12.7593 10.75 12.9744V14.3264H12.896C12.9455 14.3263 12.9939 14.3421 13.0351 14.3718C13.0763 14.4015 13.1084 14.4438 13.1274 14.4933C13.1464 14.5427 13.1513 14.5972 13.1416 14.6497C13.1319 14.7022 13.1081 14.7504 13.073 14.7882L10.177 17.9206C10.1538 17.9457 10.1262 17.9657 10.0958 17.9794C10.0654 17.993 10.0329 18 10 18C9.96712 18 9.93456 17.993 9.90418 17.9794C9.87381 17.9657 9.84622 17.9457 9.823 17.9206L6.927 14.7882C6.89195 14.7504 6.86806 14.7022 6.85838 14.6497C6.8487 14.5972 6.85365 14.5427 6.87261 14.4933C6.89157 14.4438 6.92368 14.4015 6.96487 14.3718C7.00607 14.3421 7.05449 14.3263 7.104 14.3264H9.25V12.9744ZM4.25 10.8112C4.44891 10.8112 4.63968 10.7257 4.78033 10.5736C4.92098 10.4215 5 10.2151 5 10C5 9.78486 4.92098 9.57852 4.78033 9.42639C4.63968 9.27426 4.44891 9.1888 4.25 9.1888H3.75C3.55109 9.1888 3.36032 9.27426 3.21967 9.42639C3.07902 9.57852 3 9.78486 3 10C3 10.2151 3.07902 10.4215 3.21967 10.5736C3.36032 10.7257 3.55109 10.8112 3.75 10.8112H4.25ZM8 10C8 10.2151 7.92098 10.4215 7.78033 10.5736C7.63968 10.7257 7.44891 10.8112 7.25 10.8112H6.75C6.55109 10.8112 6.36032 10.7257 6.21967 10.5736C6.07902 10.4215 6 10.2151 6 10C6 9.78486 6.07902 9.57852 6.21967 9.42639C6.36032 9.27426 6.55109 9.1888 6.75 9.1888H7.25C7.44891 9.1888 7.63968 9.27426 7.78033 9.42639C7.92098 9.57852 8 9.78486 8 10ZM10.25 10.8112C10.4489 10.8112 10.6397 10.7257 10.7803 10.5736C10.921 10.4215 11 10.2151 11 10C11 9.78486 10.921 9.57852 10.7803 9.42639C10.6397 9.27426 10.4489 9.1888 10.25 9.1888H9.75C9.55109 9.1888 9.36032 9.27426 9.21967 9.42639C9.07902 9.57852 9 9.78486 9 10C9 10.2151 9.07902 10.4215 9.21967 10.5736C9.36032 10.7257 9.55109 10.8112 9.75 10.8112H10.25ZM14 10C14 10.2151 13.921 10.4215 13.7803 10.5736C13.6397 10.7257 13.4489 10.8112 13.25 10.8112H12.75C12.5511 10.8112 12.3603 10.7257 12.2197 10.5736C12.079 10.4215 12 10.2151 12 10C12 9.78486 12.079 9.57852 12.2197 9.42639C12.3603 9.27426 12.5511 9.1888 12.75 9.1888H13.25C13.4489 9.1888 13.6397 9.27426 13.7803 9.42639C13.921 9.57852 14 9.78486 14 10ZM16.25 10.8112C16.4489 10.8112 16.6397 10.7257 16.7803 10.5736C16.921 10.4215 17 10.2151 17 10C17 9.78486 16.921 9.57852 16.7803 9.42639C16.6397 9.27426 16.4489 9.1888 16.25 9.1888H15.75C15.5511 9.1888 15.3603 9.27426 15.2197 9.42639C15.079 9.57852 15 9.78486 15 10C15 10.2151 15.079 10.4215 15.2197 10.5736C15.3603 10.7257 15.5511 10.8112 15.75 10.8112H16.25Z"
														fill="none"
														class="fill-zinc-100 p-4 group-hover:fill-zinc-200 "
													/>
												</svg>

												<svg
													fill="none"
													xmlns="http://www.w3.org/2000/svg"
													class="group hidden h-8 w-8 rounded p-1.5 peer-checked:block"
												>
													<path
														d="M10.177 2.07944L13.073 5.21176C13.1081 5.24957 13.1319 5.2978 13.1416 5.35031C13.1513 5.40283 13.1464 5.45727 13.1274 5.50674C13.1084 5.55621 13.0763 5.59848 13.0351 5.62818C12.9939 5.65789 12.9455 5.67369 12.896 5.6736H10.75V7.0256C10.75 7.24074 10.671 7.44707 10.5303 7.5992C10.3897 7.75133 10.1989 7.8368 10 7.8368C9.80109 7.8368 9.61032 7.75133 9.46967 7.5992C9.32902 7.44707 9.25 7.24074 9.25 7.0256V5.6736H7.104C7.05449 5.67369 7.00607 5.65789 6.96487 5.62818C6.92368 5.59848 6.89157 5.55621 6.87261 5.50674C6.85365 5.45727 6.8487 5.40283 6.85838 5.35031C6.86806 5.2978 6.89195 5.24957 6.927 5.21176L9.823 2.07944C9.84622 2.05426 9.87381 2.03428 9.90418 2.02065C9.93456 2.00702 9.96712 2 10 2C10.0329 2 10.0654 2.00702 10.0958 2.02065C10.1262 2.03428 10.1538 2.05426 10.177 2.07944ZM9.25 12.9744C9.25 12.7593 9.32902 12.5529 9.46967 12.4008C9.61032 12.2487 9.80109 12.1632 10 12.1632C10.1989 12.1632 10.3897 12.2487 10.5303 12.4008C10.671 12.5529 10.75 12.7593 10.75 12.9744V14.3264H12.896C12.9455 14.3263 12.9939 14.3421 13.0351 14.3718C13.0763 14.4015 13.1084 14.4438 13.1274 14.4933C13.1464 14.5427 13.1513 14.5972 13.1416 14.6497C13.1319 14.7022 13.1081 14.7504 13.073 14.7882L10.177 17.9206C10.1538 17.9457 10.1262 17.9657 10.0958 17.9794C10.0654 17.993 10.0329 18 10 18C9.96712 18 9.93456 17.993 9.90418 17.9794C9.87381 17.9657 9.84622 17.9457 9.823 17.9206L6.927 14.7882C6.89195 14.7504 6.86806 14.7022 6.85838 14.6497C6.8487 14.5972 6.85365 14.5427 6.87261 14.4933C6.89157 14.4438 6.92368 14.4015 6.96487 14.3718C7.00607 14.3421 7.05449 14.3263 7.104 14.3264H9.25V12.9744ZM4.25 10.8112C4.44891 10.8112 4.63968 10.7257 4.78033 10.5736C4.92098 10.4215 5 10.2151 5 10C5 9.78486 4.92098 9.57852 4.78033 9.42639C4.63968 9.27426 4.44891 9.1888 4.25 9.1888H3.75C3.55109 9.1888 3.36032 9.27426 3.21967 9.42639C3.07902 9.57852 3 9.78486 3 10C3 10.2151 3.07902 10.4215 3.21967 10.5736C3.36032 10.7257 3.55109 10.8112 3.75 10.8112H4.25ZM8 10C8 10.2151 7.92098 10.4215 7.78033 10.5736C7.63968 10.7257 7.44891 10.8112 7.25 10.8112H6.75C6.55109 10.8112 6.36032 10.7257 6.21967 10.5736C6.07902 10.4215 6 10.2151 6 10C6 9.78486 6.07902 9.57852 6.21967 9.42639C6.36032 9.27426 6.55109 9.1888 6.75 9.1888H7.25C7.44891 9.1888 7.63968 9.27426 7.78033 9.42639C7.92098 9.57852 8 9.78486 8 10ZM10.25 10.8112C10.4489 10.8112 10.6397 10.7257 10.7803 10.5736C10.921 10.4215 11 10.2151 11 10C11 9.78486 10.921 9.57852 10.7803 9.42639C10.6397 9.27426 10.4489 9.1888 10.25 9.1888H9.75C9.55109 9.1888 9.36032 9.27426 9.21967 9.42639C9.07902 9.57852 9 9.78486 9 10C9 10.2151 9.07902 10.4215 9.21967 10.5736C9.36032 10.7257 9.55109 10.8112 9.75 10.8112H10.25ZM14 10C14 10.2151 13.921 10.4215 13.7803 10.5736C13.6397 10.7257 13.4489 10.8112 13.25 10.8112H12.75C12.5511 10.8112 12.3603 10.7257 12.2197 10.5736C12.079 10.4215 12 10.2151 12 10C12 9.78486 12.079 9.57852 12.2197 9.42639C12.3603 9.27426 12.5511 9.1888 12.75 9.1888H13.25C13.4489 9.1888 13.6397 9.27426 13.7803 9.42639C13.921 9.57852 14 9.78486 14 10ZM16.25 10.8112C16.4489 10.8112 16.6397 10.7257 16.7803 10.5736C16.921 10.4215 17 10.2151 17 10C17 9.78486 16.921 9.57852 16.7803 9.42639C16.6397 9.27426 16.4489 9.1888 16.25 9.1888H15.75C15.5511 9.1888 15.3603 9.27426 15.2197 9.42639C15.079 9.57852 15 9.78486 15 10C15 10.2151 15.079 10.4215 15.2197 10.5736C15.3603 10.7257 15.5511 10.8112 15.75 10.8112H16.25Z"
														fill="none"
														class="fill-zinc-600 p-4 group-hover:fill-zinc-200 "
													/>
												</svg>
											</label>
										</button>
										{#if !fullContext}
											<input
												type="number"
												bind:value={context}
												class="w-14 rounded py-1 pl-2 pr-1"
											/>
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
