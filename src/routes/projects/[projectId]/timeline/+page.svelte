<script lang="ts">
	import { themeIcons } from 'seti-icons';
	import type { PageData } from './$types';
	import { listFiles } from '$lib/sessions';
	import type { Delta } from '$lib/deltas';
	import { toHumanBranchName } from '$lib/branch';
	import { add, format, differenceInSeconds, addSeconds } from 'date-fns';
	import { Slider } from 'fluent-svelte';
	import TimelineDaySessionActivities from '$lib/components/timeline/TimelineDaySessionActivities.svelte';
	import { CodeViewer } from '$lib/components';
	import 'fluent-svelte/theme.css';
	import type { UISession } from '$lib/uisessions';

	export let data: PageData;
	const { project, dateSessions } = data;

	const formatDate = (date: Date) => {
		return new Intl.DateTimeFormat('default', {
			weekday: 'short',
			day: 'numeric',
			month: 'short'
		}).format(date);
	};

	const formatTime = (date: Date) => {
		return new Intl.DateTimeFormat('default', {
			hour: 'numeric',
			minute: 'numeric'
		}).format(date);
	};

	function pathToName(path: string) {
		return path.split('/').slice(-1)[0];
	}

	const getIcon = themeIcons({
		blue: '#268bd2',
		grey: '#657b83',
		'grey-light': '#839496',
		green: '#859900',
		orange: '#cb4b16',
		pink: '#d33682',
		purple: '#6c71c4',
		red: '#dc322f',
		white: '#fdf6e3',
		yellow: '#b58900',
		ignore: '#586e75'
	});

	function pathToIconSvg(path: string) {
		let name: string = pathToName(path);
		let { svg } = getIcon(name);
		return svg;
	}

	type Selection = {
		sessionIdx: number;
		dateMilliseconds: number;
		branch: string;
		start: Date;
		end: Date;
		deltas: Record<string, Delta[]>;
		files: Promise<Record<string, string>>;
		selectedFilePath: string;
		selectedColumn: number;
	};
	$: selection = {} as Selection;

	const resetSelection = () => {
		selection = {} as Selection;
	};

	function scrollExpandedIntoView(dateMilliseconds: number) {
		new Promise((r) => setTimeout(r, 10)).then(() => {
			const element = document.getElementById(dateMilliseconds.toString());
			if (element) {
				element.scrollIntoView({
					behavior: 'smooth',
					block: 'center',
					inline: 'center'
				});
			}
		});
	}

	const timeStampToCol = (deltaTimestamp: Date, start: Date, end: Date) => {
		if (deltaTimestamp < start || deltaTimestamp > end) {
			console.error(
				`Delta timestamp out of session range. Delta timestamp: ${deltaTimestamp}, Session start: ${start}, Session end: ${end}`
			);
		}
		// there are 88 columns
		// start is column 17
		const totalDiff = differenceInSeconds(end, start);
		const eventDiff = differenceInSeconds(deltaTimestamp, start);
		const rat = eventDiff / totalDiff;
		const col = Math.floor(rat * 63 + 17);
		return col;
	};

	const colToTimestamp = (col: number, start: Date, end: Date) => {
		const totalDiff = differenceInSeconds(end, start);
		const colDiff = col - 17;
		const rat = colDiff / 63;
		const eventDiff = totalDiff * rat;
		const timestamp = addSeconds(start, eventDiff);
		return timestamp;
	};

	const sliderValueTimestampMs = (selection: Selection) =>
		colToTimestamp(selection.selectedColumn, selection.start, selection.end).getTime() +
		Math.floor((selection.end.getTime() - selection.start.getTime()) / 63); // how many ms each column represents

	// Returns a shortened version of the file path where each directory is shortened to the first three characters, except for the last directory
	const shortenFilePath = (filePath: string) => {
		const split = filePath.split('/');
		const shortened = split.map((dir, i) => {
			if (i === split.length - 1) return dir;
			return dir.slice(0, 3);
		});
		return shortened.join('/');
	};

	const expandSession = (
		idx: number,
		uiSession: UISession,
		dateMilliseconds: number,
		selectedFilePath?: string
	) => {
		selection = {
			sessionIdx: idx,
			dateMilliseconds: dateMilliseconds,
			branch: uiSession.session.meta.branch || 'master',
			start: new Date(uiSession.earliestDeltaTimestampMs),
			end: addSeconds(new Date(uiSession.latestDeltaTimestampMs), 60),
			deltas: uiSession.deltas,
			files: listFiles({
				projectId: $project?.id || '',
				sessionId: uiSession.session.id,
				paths: Object.keys(uiSession.deltas)
			}),
			selectedFilePath: selectedFilePath || Object.keys(uiSession.deltas)[0],
			selectedColumn: 0
		};
		scrollExpandedIntoView(dateMilliseconds);
	};
</script>

{#if $dateSessions === undefined}
	<span>Loading...</span>
{:else}
	<div class="flex h-full flex-row space-x-12 px-4 py-4 pb-6">
		{#each Object.entries($dateSessions) as [dateMilliseconds, uiSessions]}
			<!-- Day -->
			<div
				id={dateMilliseconds}
				class="session-day-component flex flex-col rounded-lg border border-zinc-700 bg-zinc-800/50"
				class:min-w-full={selection.dateMilliseconds == +dateMilliseconds}
			>
				<div
					class="session-day-container flex items-center border-b border-zinc-700 bg-zinc-700/30 py-2 px-4 font-medium"
				>
					<span class="session-day-header font-bold text-zinc-200">
						{formatDate(new Date(+dateMilliseconds))}
					</span>
				</div>
				{#if selection.dateMilliseconds !== +dateMilliseconds}
					<div class="flex flex-auto flex-col">
						<div class="flex h-2/3 space-x-2 p-3">
							{#each uiSessions as uiSession, i}
								<!-- Session (overview) -->

								<div class="session-column-container flex w-40 flex-col">
									<!-- svelte-ignore a11y-click-events-have-key-events -->
									<div
										class="repository-name borded cursor-pointer rounded border border-orange-400 bg-orange-400  p-1 text-center text-sm font-bold text-zinc-800 hover:bg-[#fdbc87]"
										on:click={() => expandSession(i, uiSession, +dateMilliseconds)}
									>
										{toHumanBranchName(uiSession.session.meta.branch)}
									</div>

									<div id="activities">
										<div class="my-2 mx-1 bg-red-500">
											<TimelineDaySessionActivities
												activities={uiSession.session.activity}
												sessionStart={uiSession.session.meta.startTimestampMs}
												sessionEnd={uiSession.session.meta.lastTimestampMs}
											/>
										</div>
									</div>

									<div class="flex flex-col p-1" id="sessions-details">
										<div class="font-medium text-zinc-400">
											{formatTime(new Date(uiSession.earliestDeltaTimestampMs))}
											-
											{formatTime(new Date(uiSession.latestDeltaTimestampMs))}
										</div>
										<div class="text-sm text-zinc-500" title="Session duration">
											{Math.round(
												(uiSession.latestDeltaTimestampMs - uiSession.earliestDeltaTimestampMs) /
													1000 /
													60
											)} min
										</div>
										<div class="overflow-y-auto overflow-x-hidden" title="Session files">
											{#each Object.keys(uiSession.deltas) as filePath}
												<button
													on:click={() => expandSession(i, uiSession, +dateMilliseconds, filePath)}
													class="flex w-32 cursor-pointer flex-row items-center"
												>
													<div class="h-6 w-6 fill-blue-400 text-zinc-200">
														{@html pathToIconSvg(filePath)}
													</div>
													<div class="file-name w-24 truncate text-zinc-300 hover:text-zinc-50">
														{pathToName(filePath)}
													</div>
												</button>
											{/each}
										</div>
									</div>
								</div>
							{/each}
						</div>
						<div class="day-summary-container h-1/3 border-t border-zinc-400 p-4 ">
							<div class="day-summary-header font-bold text-zinc-200">Day summary</div>
						</div>
					</div>
				{:else}
					<div class="my-2 flex flex-auto flex-row space-x-2 overflow-auto">
						<div class="">
							<button
								on:click={() => {
									if (selection.sessionIdx > 0) {
										expandSession(
											selection.sessionIdx - 1,
											uiSessions[selection.sessionIdx - 1],
											+dateMilliseconds
										);
									}
								}}
								class="{selection.sessionIdx == 0
									? 'disabled cursor-default brightness-50'
									: 'hover:bg-[#fdbc87]'} rounded-r border border-orange-400 bg-orange-400 p-1 text-center text-sm font-medium text-zinc-800 "
							>
								‹
							</button>
						</div>
						<div class="flex w-full flex-col rounded-t border border-orange-400">
							<div
								class="session-header flex items-center justify-between rounded-t-sm border border-orange-400 bg-orange-400 p-1 px-4 text-sm font-bold text-zinc-800"
							>
								<span class="cursor-default"
									>{format(selection.start, 'hh:mm')} - {format(selection.end, 'hh:mm')}</span
								>
								<span>{toHumanBranchName(selection.branch)}</span>
								<button on:click={resetSelection}>Close</button>
							</div>
							<div class="timeline-container flex flex-auto flex-col overflow-auto">
								<div class="mb-1 shadow shadow-zinc-700 ring-1 ring-zinc-700 ring-opacity-5">
									<div class="-mr-px grid  grid-cols-11  border-zinc-700 text-xs font-medium">
										<div class="col-span-2 flex items-center justify-center py-1">
											<span>{format(selection.start, 'hh:mm')}</span>
										</div>
										<div class="col-span-2 flex items-center justify-center py-1">
											<span>
												{format(
													add(selection.start, {
														seconds: differenceInSeconds(selection.end, selection.start) * 0.25
													}),
													'hh:mm'
												)}
											</span>
										</div>
										<div class="col-span-2 flex items-center justify-center py-1">
											<span>
												{format(
													add(selection.start, {
														seconds: differenceInSeconds(selection.end, selection.start) * 0.5
													}),
													'hh:mm'
												)}
											</span>
										</div>
										<div class="col-span-2 flex items-center justify-center py-1">
											<span>
												{format(
													add(selection.start, {
														seconds: differenceInSeconds(selection.end, selection.start) * 0.75
													}),
													'hh:mm'
												)}
											</span>
										</div>
										<div class="col-span-2 flex items-center justify-center py-1">
											<span>{format(selection.end, 'hh:mm')}</span>
										</div>
									</div>
									<!-- needle -->
									<div class="grid grid-cols-11">
										<div class="col-span-2 flex items-center justify-center" />
										<div class="col-span-8 -mx-1 flex items-center justify-center">
											<Slider min={17} max={80} step={1} bind:value={selection.selectedColumn}>
												<svelte:fragment slot="tooltip" let:value>
													{format(colToTimestamp(value, selection.start, selection.end), 'hh:mm')}
												</svelte:fragment>
											</Slider>
										</div>
										<div class="col-span-1 flex items-center justify-center" />
									</div>
								</div>
								<div class="timeline-file-list flex mb-1 border-b-zinc-700 border-b-2">
									<div class="grid flex-auto grid-cols-1 grid-rows-1">
										<!-- file names list -->
										<div
											class="bg-col-start-1 col-end-2 row-start-1 grid divide-y divide-zinc-700/20"
											style="grid-template-rows: repeat({Object.keys(selection.deltas)
												.length}, minmax(1rem, 1fr));"
										>
											<!-- <div class="row-end-1 h-7" /> -->

											{#each Object.keys(selection.deltas) as filePath}
												<div
													class="flex {filePath === selection.selectedFilePath
														? 'mx-1 rounded-sm bg-blue-500/70 font-bold'
														: ''}"
												>
													<button
														class="sticky left-0 z-20 flex w-1/6 items-center justify-end overflow-hidden text-xs leading-5 
                                                                        {selection.selectedFilePath ===
														filePath
															? 'cursor-default text-zinc-200'
															: 'cursor-pointer text-zinc-400 hover:text-zinc-200'}"
														on:click={() => (selection.selectedFilePath = filePath)}
														title={filePath}
													>
														{shortenFilePath(filePath)}
													</button>
												</div>
											{/each}
										</div>

										<!-- col selection -->
										<div
											class="col-start-1 col-end-2 row-start-1 grid"
											style="grid-template-columns: repeat(88, minmax(0, 1fr));"
										>
											<div
												class="bg-orange-400/40 "
												style=" grid-column: {selection.selectedColumn};"
											/>
										</div>
										<!-- time vertical lines -->
										<div
											class="col-start-1 col-end-2 row-start-1 grid grid-cols-11 grid-rows-1 divide-x divide-zinc-700/50"
										>
											<div class="col-span-2 row-span-full" />
											<div class="col-span-2 row-span-full" />
											<div class="col-span-2 row-span-full" />
											<div class="col-span-2 row-span-full" />
											<div class="col-span-2 row-span-full" />
											<div class="col-span-2 row-span-full" />
										</div>

										<!-- actual entries  -->
										<ol
											class="col-start-1 col-end-2 row-start-1 grid"
											style="
                                                                grid-template-columns: repeat(88, minmax(0, 1fr));
                                                                grid-template-rows: repeat({Object.keys(
												selection.deltas
											).length}, minmax(0px, 1fr)) auto;"
										>
											{#each Object.entries(selection.deltas) as [filePath, fileDeltas], idx}
												{#each fileDeltas as delta}
													<li
														class="relative m-0.5 flex cursor-pointer items-center rounded bg-zinc-300 hover:bg-zinc-100"
														style="
                                                                            grid-row: {idx +
															1} / span 1;
                                                                            grid-column: {timeStampToCol(
															new Date(delta.timestampMs),
															selection.start,
															selection.end
														)} / span 1;"
													>
														<button
															class="z-20 flex w-full flex-col items-center justify-center"
															on:click={() => {
																selection.selectedColumn = timeStampToCol(
																	new Date(delta.timestampMs),
																	selection.start,
																	selection.end
																);
																selection.selectedFilePath = filePath;
															}}
														/>
													</li>
												{/each}
											{/each}
										</ol>
									</div>
								</div>
								{#await selection.files then files}
									<div class="flex flex-auto overflow-auto">
										<CodeViewer
											doc={files[selection.selectedFilePath] || ''}
											deltas={selection.deltas[selection.selectedFilePath].filter(
												(delta) => delta.timestampMs <= sliderValueTimestampMs(selection)
											)}
											filepath={selection.selectedFilePath}
										/>
									</div>
								{/await}
							</div>
						</div>
						<div class="">
							<button
								on:click={() => {
									if (selection.sessionIdx < uiSessions.length - 1) {
										expandSession(
											selection.sessionIdx + 1,
											uiSessions[selection.sessionIdx + 1],
											+dateMilliseconds
										);
									}
								}}
								class="{selection.sessionIdx < uiSessions.length - 1
									? 'hover:bg-[#fdbc87]'
									: 'disabled cursor-default brightness-50'} rounded-r border border-orange-400 bg-orange-400 p-1 text-center text-sm font-medium text-zinc-800 "
							>
								›
							</button>
						</div>
					</div>
				{/if}
			</div>
		{/each}
	</div>
{/if}
