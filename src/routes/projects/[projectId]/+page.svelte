<script lang="ts">
	import type { LayoutData } from './$types';

	export let data: LayoutData;
	$: project = data.project;
	$: dateSessions = data.dateSessions;
	$: filesStatus = data.filesStatus;

	function shortPath(path, max = 3) {
		if (path.length < 30) {
			return path;
		}
		const pathParts = path.split('/');
		const file = pathParts.pop();
		if (pathParts.length > 0) {
			const pp = pathParts.map((p) => p.slice(0, max)).join('/');
			return `${pp}/${file}`;
		}
		return file;
	}

	// convert a list of timestamps to a sparkline
	function timestampsToSpark(tsArray) {
		let range = tsArray[0] - tsArray[tsArray.length - 1];

		let totalBuckets = 18;
		let bucketSize = range / totalBuckets;
		let buckets = [];
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
	function sessionFileMap(sessions: any[]) {
		let sessionsByFile = {};
		sessions.forEach((session) => {
			if (session.deltas) {
				Object.entries(session.deltas).forEach((deltas) => {
					let filename = deltas[0];
					let timestamps = deltas[1].map((delta: any) => {
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
	function orderedSessions(dateSessions: Record<string, any>) {
		return Object.entries(dateSessions)
			.sort((a, b) => {
				return parseInt(b[0]) - parseInt(a[0]);
			})
			.map(([date, sessions]) => {
				return [date, sessionFileMap(sessions)];
			})
			.slice(0, 3);
	}

	function recentActivity(dateSessions: Record<string, any>) {
		let recentActivity = [];
		if (dateSessions) {
			Object.entries(dateSessions).forEach(([date, sessions]) => {
				console.log(date, sessions);
				sessions.forEach((session) => {
					if (session.session) {
						session.session.activity.forEach((activity) => {
							recentActivity.push(activity);
						});
					}
				});
			});
		}
		let activitySorted = recentActivity.sort((a, b) => {
			return b.timestampMs - a.timestampMs;
		});
		console.log(activitySorted);
		return activitySorted.slice(0, 20);
	}
</script>

<div class="mt-4 px-8 flex flex-col">
	<h1 class="flex text-xl text-zinc-200">
		{$project?.title} <span class="text-zinc-600 ml-2">Project</span>
	</h1>
	<div class="grid grid-cols-3 mt-4">
		<div class="col-span-2 pr-6">
			<h2 class="text-lg font-bold text-zinc-500 mb-4">Recent File Changes</h2>
			{#if $dateSessions === undefined}
				<span>Loading...</span>
			{:else}
				<div class="flex flex-col space-y-4">
					{#each orderedSessions($dateSessions) as [dateMilliseconds, fileSessions]}
						<div class="flex flex-col">
							<div class="text-zinc-400 text-lg text-zinc-200 mb-1">
								{new Date(parseInt(dateMilliseconds)).toLocaleDateString('en-us', {
									weekday: 'long',
									year: 'numeric',
									month: 'short',
									day: 'numeric'
								})}
							</div>
							<div class="bg-zinc-700 rounded p-4">
								{#each Object.entries(fileSessions) as filetime}
									<div class="flex flex-row justify-between">
										<div class="text-zinc-100 font-mono">{filetime[0]}</div>
										<div class="text-zinc-400">{@html timestampsToSpark(filetime[1])}</div>
									</div>
								{/each}
							</div>
						</div>
					{/each}
				</div>
			{/if}
		</div>
		<div class="col-span-1 space-y-6">
			<div>
				<h2 class="text-lg font-bold text-zinc-500 mb-2">Work in Progress</h2>
				{#if Object.entries(filesStatus).length == 0}
					<div class="bg-green-900 text-green-500 p-4 rounded">Everything is committed</div>
				{:else}
					<div class="bg-blue-900 p-4 rounded">
						<ul class="">
							{#each Object.entries(filesStatus) as activity}
								<li>
									{activity[1].slice(0, 1)}
									{shortPath(activity[0])}
								</li>
							{/each}
						</ul>
					</div>
				{/if}
			</div>

			<div>
				<h2 class="text-lg font-bold text-zinc-500">Recent Activity</h2>
				{#each recentActivity($dateSessions) as activity}
					<div class="text-zinc-400 mt-4 mb-1">
						<div class="flex flex-col">
							<div class="flex flex-row justify-between p-2 bg-zinc-700 rounded-t">
								<div class="text-zinc-300">
									{new Date(parseInt(activity.timestampMs)).toLocaleDateString('en-us', {
										weekday: 'long',
										year: 'numeric',
										month: 'short',
										day: 'numeric'
									})}
								</div>
								<div>{activity.type}</div>
							</div>
							<div class="bg-zinc-600 rounded-b p-2">{activity.message}</div>
						</div>
					</div>
				{/each}
			</div>
		</div>
	</div>
</div>
