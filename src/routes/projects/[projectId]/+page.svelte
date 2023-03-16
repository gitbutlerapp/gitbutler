<script lang="ts">
	import type { LayoutData } from './$types';
	import type { Readable } from 'svelte/store';
	import type { Session } from '$lib/sessions';
	import { startOfDay } from 'date-fns';
	import type { Activity } from '$lib/sessions';
	import type { Delta } from '$lib/deltas';
	import { shortPath } from '$lib/paths';
	import { invoke } from '@tauri-apps/api';
	import { toHumanBranchName } from '$lib/branch';
	import { list as listDeltas } from '$lib/deltas';
	import { slide } from 'svelte/transition';
	import { navigating } from '$app/stores';
	import toast from 'svelte-french-toast';
	import { goto } from '$app/navigation';

	const getBranch = (params: { projectId: string }) => invoke<string>('git_branch', params);

	export let data: LayoutData;
	$: project = data.project;
	$: filesStatus = data.filesStatus;
	$: recentActivity = data.recentActivity as Readable<Activity[]>;
	$: orderedSessionsFromLastFourDays = data.orderedSessionsFromLastFourDays;

	const commit = (params: {
		projectId: string;
		message: string;
		files: Array<string>;
		push: boolean;
	}) => invoke<boolean>('git_commit', params);

	let latestDeltasByDateByFile: Record<number, Record<string, Delta[][]>[]> = {};
	let commitMessage: string;
	let initiatedCommit = false;
	let filesSelectedForCommit: string[] = [];

	$: if ($navigating) {
		commitMessage = '';
		filesSelectedForCommit = [];
		initiatedCommit = false;
	}

	function gotoPlayer(filename: string) {
		if ($project) {
			goto(`/projects/${$project.id}/player?file=${encodeURIComponent(filename)}`);
		}
	}

	function doCommit() {
		if ($project) {
			commit({
				projectId: $project.id,
				message: commitMessage,
				files: filesSelectedForCommit,
				push: false
			}).then((result) => {
				toast.success('Commit successful!', {
					icon: '🎉'
				});
				commitMessage = '';
				filesSelectedForCommit = [];
				initiatedCommit = false;
			});
		}
	}

	$: if ($project) {
		latestDeltasByDateByFile = {};
		const dateSessions: Record<number, Session[]> = {};
		$orderedSessionsFromLastFourDays.forEach((session) => {
			const date = startOfDay(new Date(session.meta.startTimestampMs));
			if (dateSessions[date.getTime()]) {
				dateSessions[date.getTime()]?.push(session);
			} else {
				dateSessions[date.getTime()] = [session];
			}
		});

		const latestDateSessions: Record<number, Session[]> = Object.fromEntries(
			Object.entries(dateSessions)
				.sort((a, b) => parseInt(b[0]) - parseInt(a[0]))
				.slice(0, 3)
		); // Only show the last 3 days

		Object.keys(latestDateSessions).forEach((date: string) => {
			Promise.all(
				latestDateSessions[parseInt(date)].map(async (session) => {
					const sessionDeltas = await listDeltas({
						projectId: $project?.id ?? '',
						sessionId: session.id
					});

					const fileDeltas: Record<string, Delta[][]> = {};

					Object.keys(sessionDeltas).forEach((filePath) => {
						if (sessionDeltas[filePath].length > 0) {
							if (fileDeltas[filePath]) {
								fileDeltas[filePath]?.push(sessionDeltas[filePath]);
							} else {
								fileDeltas[filePath] = [sessionDeltas[filePath]];
							}
						}
					});
					return fileDeltas;
				})
			).then((sessionsByFile) => {
				latestDeltasByDateByFile[parseInt(date)] = sessionsByFile;
			});
		});
	}

	let gitBranch = <string | undefined>undefined;
	$: if ($project) {
		getBranch({ projectId: $project?.id }).then((branch) => {
			gitBranch = branch;
		});
	}

	// convert a list of timestamps to a sparkline
	function timestampsToSpark(tsArray: number[]) {
		let range = tsArray[0] - tsArray[tsArray.length - 1];

		let totalBuckets = 18;
		let bucketSize = range / totalBuckets;
		let buckets: number[][] = [];
		for (let i = 0; i <= totalBuckets; i++) {
			buckets.push([]);
		}
		tsArray.forEach((ts) => {
			let bucket = Math.floor((tsArray[0] - ts) / bucketSize);
			if (bucket && ts) {
				buckets[bucket].push(ts);
			}
		});

		let spark = '';
		buckets.forEach((entries) => {
			let size = entries.length;
			if (size < 1) {
				spark += '<span class="text-zinc-600">▁</span>';
			} else if (size < 2) {
				spark += '<span class="text-blue-200">▂</span>';
			} else if (size < 3) {
				spark += '<span class="text-blue-200">▃</span>';
			} else if (size < 4) {
				spark += '<span class="text-blue-200">▄</span>';
			} else if (size < 5) {
				spark += '<span class="text-blue-200">▅</span>';
			} else if (size < 6) {
				spark += '<span class="text-blue-200">▆</span>';
			} else if (size < 7) {
				spark += '<span class="text-blue-200">▇</span>';
			} else {
				spark += '<span class="text-blue-200">█</span>';
			}
		});
		return spark;
	}

	// reduce a group of sessions to a map of filename to timestamps array
	function sessionFileMap(sessions: Record<string, Delta[][]>[]): Record<string, number[]> {
		let sessionsByFile: Record<string, number[]> = {};

		for (const s of sessions) {
			for (const [filename, deltas] of Object.entries(s)) {
				let timestamps = deltas.flatMap((d) => d.map((dd) => dd.timestampMs));
				if (sessionsByFile[filename]) {
					sessionsByFile[filename] = sessionsByFile[filename].concat(timestamps).sort();
				} else {
					sessionsByFile[filename] = timestamps;
				}
			}
		}
		return sessionsByFile;
	}

	// order the sessions and summarize the changes by file
	function orderedSessions(dateSessions: Record<number, Record<string, Delta[][]>[]>) {
		return Object.entries(dateSessions)
			.sort((a, b) => parseInt(b[0]) - parseInt(a[0]))
			.map(([date, sessions]) => {
				return [date, sessionFileMap(sessions)];
			});
	}
</script>

<div class="project-section-component" style="height: calc(100vh - 118px); overflow: hidden;">
	<div class="flex h-full">
		<div
			class="main-column-containercol-span-2 mt-4"
			style="width: calc(100% * 0.66); height: calc(-126px + 100vh)"
		>
			<h1 class="flex py-4 px-8 text-xl text-zinc-300">
				{$project?.title} <span class="ml-2 text-zinc-600">Project</span>
			</h1>
			<div class="mt-4">
				<div class="recent-file-changes-container h-full w-full">
					<h2 class="mb-4 px-8 text-lg font-bold text-zinc-300">Recently changed files</h2>
					{#if latestDeltasByDateByFile === undefined}
						<div class="p-8 text-center text-zinc-400">Loading...</div>
					{:else}
						<div
							class="flex flex-col space-y-4 overflow-y-auto px-8 pb-8"
							style="height: calc(100vh - 253px);"
						>
							{#if orderedSessions(latestDeltasByDateByFile).length == 0}
								<div class="text-zinc-400">Waiting for your first file changes...</div>
							{/if}

							{#each orderedSessions(latestDeltasByDateByFile) as [dateMilliseconds, fileSessions]}
								<div class="flex flex-col">
									<div class="mb-1  text-zinc-300">
										{new Date(parseInt(dateMilliseconds)).toLocaleDateString('en-us', {
											weekday: 'long',
											year: 'numeric',
											month: 'short',
											day: 'numeric'
										})}
									</div>
									<div
										class="results-card rounded border border-zinc-700 bg-[#2F2F33] p-4 drop-shadow-lg"
									>
										{#each Object.entries(fileSessions) as filetime}
											<div class="flex flex-row justify-between">
												<div class="font-mono text-zinc-300">
													<a class="cursor-pointer" on:click={gotoPlayer(filetime[0])}>
														{filetime[0]}
													</a>
												</div>
												<div class="font-mono text-zinc-400">
													{@html timestampsToSpark(filetime[1])}
												</div>
											</div>
										{/each}
									</div>
								</div>
							{/each}
						</div>
					{/if}
				</div>
			</div>
		</div>
		<div
			class="secondary-column-container col-span-1 flex flex-col border-l border-l-zinc-700"
			style="width: 37%;"
		>
			<div class="work-in-progress-container border-b border-zinc-700 py-4 px-4 ">
				<h2 class="mb-2 text-lg font-bold text-zinc-300">Work in Progress</h2>
				{#if gitBranch}
					<div class="w-100 mb-4 flex items-center justify-between">
						<div
							class="button group flex max-w-[200px] justify-between rounded border border-zinc-600 bg-zinc-700 py-2 px-4 text-zinc-300 shadow"
						>
							<div class="h-4 w-4">
								<svg
									text="gray"
									aria-hidden="true"
									height="16"
									viewBox="0 0 16 16"
									version="1.1"
									width="16"
									data-view-component="true"
									class="h-4 w-4 fill-zinc-400"
								>
									<path
										d="M9.5 3.25a2.25 2.25 0 1 1 3 2.122V6A2.5 2.5 0 0 1 10 8.5H6a1 1 0 0 0-1 1v1.128a2.251 2.251 0 1 1-1.5 0V5.372a2.25 2.25 0 1 1 1.5 0v1.836A2.493 2.493 0 0 1 6 7h4a1 1 0 0 0 1-1v-.628A2.25 2.25 0 0 1 9.5 3.25Zm-6 0a.75.75 0 1 0 1.5 0 .75.75 0 0 0-1.5 0Zm8.25-.75a.75.75 0 1 0 0 1.5.75.75 0 0 0 0-1.5ZM4.25 12a.75.75 0 1 0 0 1.5.75.75 0 0 0 0-1.5Z"
									/>
								</svg>
							</div>
							<div class="truncate pl-2 font-mono text-zinc-300">
								{toHumanBranchName(gitBranch)}
							</div>
							<div class="carrot flex items-center pl-3">
								<svg width="7" height="5" viewBox="0 0 7 5" fill="none" class="fill-zinc-400">
									<path
										d="M3.87796 4.56356C3.67858 4.79379 3.32142 4.79379 3.12204 4.56356L0.319371 1.32733C0.0389327 1.00351 0.268959 0.5 0.697336 0.5L6.30267 0.500001C6.73104 0.500001 6.96107 1.00351 6.68063 1.32733L3.87796 4.56356Z"
										fill="#A1A1AA"
									/>
								</svg>
							</div>
						</div>
						<div class="branch-count-container text-md hover:text-blue-500 ">6 branches</div>
					</div>
				{/if}
				{#if $filesStatus.length == 0}
					<div
						class="flex rounded border border-green-700 bg-green-900 p-4 align-middle text-green-400"
					>
						<div class="icon mr-2 h-5 w-5">
							<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20">
								<path
									fill="#4ADE80"
									fill-rule="evenodd"
									d="M2 10a8 8 0 1 0 16 0 8 8 0 0 0-16 0Zm12.16-1.44a.8.8 0 0 0-1.12-1.12L9.2 11.28 7.36 9.44a.8.8 0 0 0-1.12 1.12l2.4 2.4c.32.32.8.32 1.12 0l4.4-4.4Z"
								/>
							</svg>
						</div>
						Everything is committed
					</div>
				{:else}
					<div class="rounded border border-yellow-400 bg-yellow-500 p-4 font-mono text-yellow-900">
						<ul class="pl-4">
							{#each $filesStatus as activity}
								<li class={initiatedCommit ? '-ml-5' : 'list-disc'}>
									{#if initiatedCommit}
										<input
											type="checkbox"
											bind:group={filesSelectedForCommit}
											value={activity.path}
										/>
									{/if}
									{activity.status.slice(0, 1)}
									{shortPath(activity.path)}
								</li>
							{/each}
						</ul>
					</div>
					<!-- TODO: Button needs to be hooked up -->
					<div class="mt-2 flex flex-col">
						{#if initiatedCommit}
							<div transition:slide={{ duration: 150 }}>
								<h3 class="text-base font-semibold text-zinc-200">Commit Message</h3>
								<textarea
									rows="4"
									name="message"
									placeholder="Description of changes"
									bind:value={commitMessage}
									class="mb-2 block w-full rounded-md border-0 p-4 text-zinc-200 ring-1 ring-inset ring-gray-600 placeholder:text-gray-400 focus:ring-2 focus:ring-inset focus:ring-blue-600 sm:py-1.5 sm:text-sm sm:leading-6"
								/>
							</div>
						{/if}
						<div class="w-100 flex flex-row-reverse items-center justify-between gap-4">
							{#if initiatedCommit}
								<div class="flex gap-2">
									<button
										class="button w-[60px] rounded border border-zinc-600 py-2 text-white hover:bg-zinc-800"
										on:click={() => {
											initiatedCommit = false;
										}}>✘</button
									>
									<button
										disabled={!commitMessage || filesSelectedForCommit.length == 0}
										class="{!commitMessage || filesSelectedForCommit.length == 0
											? 'bg-zinc-800 text-zinc-600'
											: ''} button rounded bg-blue-600 py-2 px-3 text-white"
										on:click={() => {
											doCommit();
											initiatedCommit = false;
											commitMessage = '';
										}}>Commit changes</button
									>
								</div>
								<div class="w-100 align-left">
									{#if filesSelectedForCommit.length == 0}
										<div>Select at least one file.</div>
									{:else if !commitMessage}
										<div>Provide a commit message.</div>
									{:else}
										<div>Are you certain of this?</div>
									{/if}
								</div>
							{:else}
								<button
									class="button rounded bg-blue-600 py-2 px-3 text-white hover:bg-blue-700"
									on:click={() => {
										filesSelectedForCommit = $filesStatus.map((file) => {
											return file.path;
										});

										initiatedCommit = true;
									}}>Commit changes</button
								>
							{/if}
						</div>
					</div>
				{/if}
			</div>
			<div
				class="recent-activity-container p-4"
				style="height: calc(100vh - 110px); overflow-y: auto;"
			>
				<h2 class="text-lg font-bold text-zinc-300">Recent Activity</h2>
				{#if $recentActivity.length == 0}
					<div class="text-zinc-400">No activity yet.</div>
				{/if}
				{#each $recentActivity as activity}
					<div
						class="recent-activity-card mt-4 mb-1 rounded border border-zinc-700 text-zinc-400 drop-shadow-lg"
					>
						<div class="flex flex-col rounded bg-[#2F2F33] p-3">
							<div class="flex flex-row justify-between pb-2 text-zinc-500">
								<div class="">
									{new Date(activity.timestampMs).toLocaleDateString('en-us', {
										weekday: 'short',
										year: 'numeric',
										month: 'short',
										day: 'numeric'
									})}
								</div>
								<div class="text-right font-mono ">{activity.type}</div>
							</div>
							<div class="rounded-b bg-[#2F2F33] text-zinc-100">{activity.message}</div>
						</div>
					</div>
				{/each}
			</div>
		</div>
	</div>
</div>
