<script lang="ts">
	import type { LayoutData } from './$types';
	import type { Readable } from 'svelte/store';
	import type { UISession } from '$lib/uisessions';
	import type { Activity } from '$lib/sessions';
	import type { Delta } from '$lib/deltas';
	import { shortPath } from '$lib/paths';
	import { invoke } from '@tauri-apps/api';
	import { toHumanBranchName } from '$lib/branch';

	const getBranch = (params: { projectId: string }) => invoke<string>('git_branch', params);

	export let data: LayoutData;
	$: project = data.project;
	$: dateSessions = data.dateSessions as Readable<Record<number, UISession[]>>;
	$: filesStatus = data.filesStatus;
	$: recentActivity = data.recentActivity as Readable<Activity[]>;

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
	function sessionFileMap(sessions: UISession[]): Record<string, number[]> {
		let sessionsByFile: Record<string, number[]> = {};
		sessions.forEach((session) => {
			if (session.deltas) {
				Object.entries(session.deltas).forEach((deltas) => {
					let filename = deltas[0];
					let timestamps = deltas[1].map((delta: Delta) => {
						return delta.timestampMs;
					});
					if (sessionsByFile[filename]) {
						sessionsByFile[filename] = sessionsByFile[filename].concat(timestamps).sort();
					} else {
						sessionsByFile[filename] = timestamps;
					}
				});
			}
		});

		return sessionsByFile;
	}

	// order the sessions and summarize the changes by file
	function orderedSessions(dateSessions: Record<number, UISession[]>) {
		return Object.entries(dateSessions)
			.sort((a, b) => {
				return parseInt(b[0]) - parseInt(a[0]);
			})
			.map(([date, sessions]) => {
				return [date, sessionFileMap(sessions)];
			})
			.slice(0, 3);
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
					<h2 class="mb-4 px-8 text-lg font-bold text-zinc-300">Recent File Changes</h2>
					{#if $dateSessions === undefined}
						<div class="p-8 text-center text-zinc-400">Loading...</div>
					{:else}
						<div
							class="flex flex-col space-y-4 overflow-y-auto px-8 pb-8"
							style="height: calc(100vh - 253px);"
						>
							{#each orderedSessions($dateSessions) as [dateMilliseconds, fileSessions]}
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
												<div class="font-mono text-zinc-100">{filetime[0]}</div>
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
			<div class="work-in-progress-container border-b border-zinc-700 py-4 px-4">
				<h2 class="mb-2 text-lg font-bold text-zinc-300">Work in Progress</h2>
				{#if gitBranch}
					<div class="py-1">Branch: {toHumanBranchName(gitBranch)}</div>
				{/if}
				{#if $filesStatus.length == 0}
					<div
						class="flex rounded border border-green-700 bg-green-900 p-4 align-middle text-green-400"
					>
						<div class="icon mr-2 h-5 w-5">
							<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"
								><path
									fill="#4ADE80"
									fill-rule="evenodd"
									d="M2 10a8 8 0 1 0 16 0 8 8 0 0 0-16 0Zm12.16-1.44a.8.8 0 0 0-1.12-1.12L9.2 11.28 7.36 9.44a.8.8 0 0 0-1.12 1.12l2.4 2.4c.32.32.8.32 1.12 0l4.4-4.4Z"
								/></svg
							>
						</div>
						Everything is committed
					</div>
				{:else}
					<div class="rounded border border-yellow-600 bg-yellow-500 p-4 font-mono text-yellow-900">
						<ul class="pl-4">
							{#each $filesStatus as activity}
								<li class="list-disc ">
									{activity.status.slice(0, 1)}
									{shortPath(activity.path)}
									
								</li>
								
							{/each}
						</ul>
					</div>
					<!-- TODO: Button needs to be hooked up -->
					<div class="flex flex-row-reverse w-100">
						<button class="button mt-2 rounded bg-blue-600 py-2 px-3 text-white">Commit changes</button>
					</div>
				{/if}
			</div>
			<div
				class="recent-activity-container p-4"
				style="height: calc(100vh - 110px); overflow-y: auto;"
			>
				<h2 class="text-lg font-bold text-zinc-300">Recent Activity</h2>
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
