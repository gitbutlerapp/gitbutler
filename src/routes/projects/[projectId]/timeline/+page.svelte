<script lang="ts">
	import { themeIcons } from 'seti-icons';
	import type { PageData } from './$types';
	import { derived } from 'svelte/store';
	import { asyncDerived } from '@square/svelte-store';
	import type { Session } from '$lib/sessions';
	import { startOfDay } from 'date-fns';
	import { list as listDeltas } from '$lib/deltas';
	import { listFiles } from '$lib/sessions';
	import { Operation } from '$lib/deltas';
	import type { Delta } from '$lib/deltas';
	import { toHumanBranchName } from '$lib/branch';
	import { add, format, differenceInSeconds, addSeconds } from 'date-fns';
	import { Slider } from 'fluent-svelte';
	import { CodeViewer } from '$lib/components';
	import TimelineDaySessionActivities from '$lib/components/timeline/TimelineDaySessionActivities.svelte';
	import 'fluent-svelte/theme.css';

	export let data: PageData;
	const { project, sessions } = data;

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

	type UISession = {
		session: Session;
		deltas: Record<string, Delta[]>;
		earliestDeltaTimestampMs: number;
		latestDeltaTimestampMs: number;
	};

	$: dateSessions = asyncDerived([sessions], async ([sessions]) => {
		const deltas = await Promise.all(
			sessions.map((session) => {
				return listDeltas({
					projectId: $project?.id ?? '',
					sessionId: session.id
				});
			})
		);
		// Sort deltas by timestamp
		deltas.forEach((delta) => {
			Object.keys(delta).forEach((key) => {
				delta[key].sort((a, b) => a.timestampMs - b.timestampMs);
			});
		});

		const uiSessions = sessions
			.map((session, i) => {
				return { session, deltas: deltas[i] } as UISession;
			})
			.filter((uiSession) => {
				return Object.keys(uiSession.deltas).length > 0;
			});

		const dateSessions: Record<number, UISession[]> = {};
		uiSessions.forEach((uiSession) => {
			const date = startOfDay(new Date(uiSession.session.meta.startTimestampMs));
			if (dateSessions[date.getTime()]) {
				dateSessions[date.getTime()]?.push(uiSession);
			} else {
				dateSessions[date.getTime()] = [uiSession];
			}
		});

		// For each UISession in dateSessions, set the earliestDeltaTimestampMs and latestDeltaTimestampMs
		Object.keys(dateSessions).forEach((date: any) => {
			dateSessions[date].forEach((uiSession: any) => {
				const deltaTimestamps = Object.keys(uiSession.deltas).reduce((acc, key) => {
					return acc.concat(uiSession.deltas[key].map((delta: Delta) => delta.timestampMs));
				}, []);
				uiSession.earliestDeltaTimestampMs = Math.min(...deltaTimestamps);
				uiSession.latestDeltaTimestampMs = Math.max(...deltaTimestamps);
			});
		});

		return dateSessions;
	});

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
	let selection = {} as Selection;

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

	let animatingOut = false;

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

<div class="h-full">
	<div class="overflow-x-hidden w-full h-full">
		<div class="h-full">
			{#if $dateSessions === undefined}
				<span>Loading...</span>
			{:else}
				<div class="h-full flex-auto flex flex-row overflow-x-auto space-x-12 px-4 py-4 pb-6">
					{#each Object.entries($dateSessions) as [dateMilliseconds, uiSessions]}
						<!-- Day -->
						<div
							id={dateMilliseconds}
							class="bg-zinc-800/50 rounded-xl border border-zinc-700 
                                flex flex-col h-full
                                {selection.dateMilliseconds == +dateMilliseconds
								? 'min-w-full overflow-hidden'
								: ''}
                                "
						>
							<div
								class="font-medium border-b border-zinc-700 bg-zinc-700/30 h-6 flex items-center pl-4"
							>
								<span class={animatingOut ? 'animate-pulse text-orange-300' : ''}>
									{formatDate(new Date(+dateMilliseconds))}
								</span>
							</div>
							{#if selection.dateMilliseconds !== +dateMilliseconds}
								<div class="h-full flex flex-col">
									<div class="h-2/3 flex space-x-2 px-4">
										{#each uiSessions as uiSession, i}
											<!-- Session (overview) -->

											<div class="flex flex-col py-2 w-40">
												<!-- svelte-ignore a11y-click-events-have-key-events -->
												<div
													class="
                                                cursor-pointer
                                                text-sm text-center font-medium rounded borded  text-zinc-800 p-1 border bg-orange-400 border-orange-400 hover:bg-[#fdbc87]"
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

												<div
													class="flex flex-col h-full overflow-y-hidden p-1"
													id="sessions-details"
												>
													<div class="text-zinc-400 font-medium">
														{formatTime(new Date(uiSession.earliestDeltaTimestampMs))}
														-
														{formatTime(new Date(uiSession.latestDeltaTimestampMs))}
													</div>
													<div class="text-zinc-500 text-sm" title="Session duration">
														{Math.round(
															(uiSession.latestDeltaTimestampMs -
																uiSession.earliestDeltaTimestampMs) /
																1000 /
																60
														)} min
													</div>
													<div class="overflow-y-auto overflow-x-hidden" title="Session files">
														{#each Object.keys(uiSession.deltas) as filePath}
															<button
																on:click={() =>
																	expandSession(i, uiSession, +dateMilliseconds, filePath)}
																class="cursor-pointer flex flex-row w-32 items-center"
															>
																<div class="w-6 h-6 text-zinc-200 fill-blue-400">
																	{@html pathToIconSvg(filePath)}
																</div>
																<div class="text-zinc-400 hover:text-zinc-200 w-24 truncate">
																	{pathToName(filePath)}
																</div>
															</button>
														{/each}
													</div>
												</div>
											</div>
										{/each}
									</div>
									<div class="h-1/3 flex-grow  px-4 border-t border-zinc-700 ">Day summary</div>
								</div>
							{:else}
								<div class="mt-2 h-full flex flex-row space-x-2">
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
												: 'hover:bg-[#fdbc87]'} rounded-r bg-orange-400 border border-orange-400 text-zinc-800 p-1 text-center text-sm font-medium "
										>
											‹
										</button>
									</div>
									<div class="flex-grow border-t border-l border-r rounded-t border-orange-400">
										<div
											class="px-4 bg-orange-400 border-t border-l border-orange-400 p-1 rounded-t-sm text-zinc-800 text-sm font-medium flex items-center justify-between"
										>
											<span class="cursor-default"
												>{format(selection.start, 'hh:mm')} - {format(selection.end, 'hh:mm')}</span
											>
											<span>{toHumanBranchName(selection.branch)}</span>
											<button on:click={resetSelection}>Close</button>
										</div>

										<div
											class="flex flex-col flex-none max-w-full select-none h-full overflow-auto"
										>
											<div class="flex flex-col flex-none max-w-full mb-40">
												<!-- sticky header -->
												<div
													class="overflow-hidden sticky top-0 z-30 bg-zinc-800 flex-none shadow shadow-zinc-700 ring-1 ring-zinc-700 ring-opacity-5 mb-1"
												>
													<div
														class="grid-cols-11 -mr-px  border-zinc-700  grid text-xs font-medium"
													>
														<div />
														<div class="col-span-2 flex items-center justify-center py-1">
															<span>{format(selection.start, 'hh:mm')}</span>
														</div>
														<div class="col-span-2 flex items-center justify-center py-1">
															<span
																>{format(
																	add(selection.start, {
																		seconds:
																			differenceInSeconds(selection.end, selection.start) * 0.25
																	}),
																	'hh:mm'
																)}</span
															>
														</div>
														<div class="col-span-2 flex items-center justify-center py-1">
															<span
																>{format(
																	add(selection.start, {
																		seconds:
																			differenceInSeconds(selection.end, selection.start) * 0.5
																	}),
																	'hh:mm'
																)}</span
															>
														</div>
														<div class="col-span-2 flex items-center justify-center py-1">
															<span
																>{format(
																	add(selection.start, {
																		seconds:
																			differenceInSeconds(selection.end, selection.start) * 0.75
																	}),
																	'hh:mm'
																)}</span
															>
														</div>
														<div class="col-span-2 flex items-center justify-center py-1">
															<span>{format(selection.end, 'hh:mm')}</span>
														</div>
													</div>
													<!-- needle -->
													<div class="grid grid-cols-11">
														<div class="col-span-2 flex items-center justify-center" />
														<div class="-mx-1 col-span-8 flex items-center justify-center">
															<Slider
																min={17}
																max={80}
																step={1}
																bind:value={selection.selectedColumn}
															>
																<svelte:fragment slot="tooltip" let:value>
																	{format(
																		colToTimestamp(value, selection.start, selection.end),
																		'hh:mm'
																	)}
																</svelte:fragment>
															</Slider>
														</div>
														<div class="col-span-1 flex items-center justify-center" />
													</div>
												</div>
												<div class="flex flex-auto mb-1">
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
																		? 'bg-zinc-500/70'
																		: ''}"
																>
																	<button
																		class="text-xs z-20 flex justify-end items-center overflow-hidden sticky left-0 w-1/6 leading-5 
                                                                        {selection.selectedFilePath ===
																		filePath
																			? 'text-zinc-200 cursor-default'
																			: 'text-zinc-400 hover:text-zinc-200 cursor-pointer'}"
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
															class="col-start-1 col-end-2 row-start-1 grid-rows-1 divide-x divide-zinc-700/50 grid grid-cols-11"
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
																		class="relative flex items-center bg-zinc-300 hover:bg-zinc-100 rounded m-0.5 cursor-pointer"
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
																			class="z-20 h-full flex flex-col w-full items-center justify-center"
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
												<div class="grid grid-cols-11 mt-6">
													<div class="col-span-2" />
													<div class="col-span-8  bg-zinc-500/70 rounded select-text">
														{#await selection.files then files}
															<CodeViewer
																doc={files[selection.selectedFilePath]}
																deltas={selection.deltas[selection.selectedFilePath]}
																end={sliderValueTimestampMs(selection)}
															/>
														{/await}
													</div>
													<div class="" />
												</div>
											</div>
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
												: 'disabled cursor-default brightness-50'} rounded-r bg-orange-400 border border-orange-400 text-zinc-800 p-1 text-center text-sm font-medium "
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
		</div>
	</div>
</div>
