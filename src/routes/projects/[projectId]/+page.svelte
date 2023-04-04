<script lang="ts">
	import { format, getTime, isEqual, startOfDay, subDays } from 'date-fns';
	import { collapsable } from '$lib/paths';
	import type { PageData } from './$types';
	import { derived } from 'svelte/store';
	import { IconGitBranch } from '$lib/components/icons';
	import type { Session } from '$lib/sessions';
	import { asyncDerived } from '@square/svelte-store';
	import { list as listDeltas, type Delta } from '$lib/deltas';
	import IconRotateClockwise from '$lib/components/icons/IconRotateClockwise.svelte';
	import FileActivity from './FileActivity.svelte';
	import { Button, Tooltip } from '$lib/components';

	export let data: PageData;
	$: activity = derived(data.activity, (activity) => activity);
	$: project = derived(data.project, (project) => project);
	$: statuses = derived(data.statuses, (statuses) => statuses);
	$: sessions = derived(data.sessions, (sessions) => sessions);
	$: head = derived(data.head, (head) => head);

	$: recentSessions = derived(
		sessions,
		(sessions) => {
			const lastFourDaysOfSessions = sessions.filter(
				(session) => session.meta.startTimestampMs >= getTime(subDays(new Date(), 4))
			);
			if (lastFourDaysOfSessions.length >= 4) return lastFourDaysOfSessions;
			return sessions.slice(0, 4);
		},
		[]
	);

	$: recentActivity = derived(
		[activity, recentSessions],
		([activity, recentSessions]) => {
			return activity
				.filter((a) => a.timestampMs >= (recentSessions.at(-1)?.meta.startTimestampMs ?? 0))
				.sort((a, b) => b.timestampMs - a.timestampMs);
		},
		[]
	);

	$: sessionByDates = derived(
		recentSessions,
		(sessions) =>
			sessions.reduce((list: [Session[], Date][], session) => {
				const date = startOfDay(new Date(session.meta.startTimestampMs));
				if (list.length === 0) {
					list.push([[session], date]);
				} else {
					const last = list[list.length - 1];
					if (isEqual(last[1], date)) {
						last[0].push(session);
					} else {
						list.push([[session], date]);
					}
				}
				return list;
			}, []),
		[]
	);

	$: filesActivityByDate = asyncDerived(
		[project, sessionByDates],
		async ([project, sessionByDates]) =>
			await Promise.all(
				sessionByDates.map(async ([sessions, date]) => {
					const deltas = await Promise.all(
						sessions.map((session) =>
							listDeltas({
								projectId: project.id,
								sessionId: session.id
							})
						)
					);
					const merged: Record<string, Delta[]> = {};
					deltas.forEach((delta) =>
						Object.entries(delta).forEach(([filepath, deltas]) => {
							if (merged[filepath]) {
								merged[filepath].push(...deltas);
							} else {
								merged[filepath] = deltas;
							}
						})
					);
					return [merged, date] as [Record<string, Delta[]>, Date];
				})
			),
		{ initial: [] }
	);
</script>

<div id="project-overview" class="flex h-full w-full">
	<div class="flex w-2/3 flex-col gap-4">
		<h1 class="flex px-8 pt-4 text-xl text-zinc-300">
			<span>{$project?.title}</span>
			<span class="ml-2 text-zinc-600">Project</span>
		</h1>

		<h2 class="px-8 text-lg font-bold text-zinc-300">Recently changed files</h2>

		<ul class="mr-1 flex flex-col space-y-4 overflow-y-auto pl-8 pr-5 pb-8">
			{#await filesActivityByDate.load()}
				<li>
					<IconRotateClockwise class="animate-spin" />
				</li>
			{:then}
				{#each $filesActivityByDate as [activity, date]}
					<li class="card changed-day-card flex flex-col border-[0.5px] border-gb-700 bg-card-default rounded">
						<header class="header flex flex-row justify-between px-3 py-2 gap-2 bg-card-active border-b-gb-700 rounded-tl rounded-tr">
							<div class="mb-1 text-zinc-300 ">
								{date.toLocaleDateString('en-us', {
									weekday: 'long',
									year: 'numeric',
									month: 'short',
									day: 'numeric'
								})}
							</div>
							<Button
								href="/projects/{$project.id}/player/{format(date, 'yyyy-MM-dd')}"
								filled={false}
								role="primary"
							>
								Replay Changes
							</Button>
						</header>
						<ul class="all-files-changed flex flex-col rounded p-4">
							{#each Object.entries(activity) as [filepath, deltas]}
								<li class="changed-file flex items-center justify-between gap-4 mb-1">
									<a
										class="file-name flex w-[50%] overflow-auto font-mono hover:underline"
										href="/projects/{$project.id}/player/{format(
											date,
											'yyyy-MM-dd'
										)}?file={encodeURIComponent(filepath)}"
									>
										<span
											class="w-full truncate"
											use:collapsable={{ value: filepath, separator: '/' }}
										/>
									</Button>
									<FileActivity {deltas} />
								</li>
							{/each}
						</ul>
					</li>
				{:else}
					<li class="text-zinc-400">
						Waiting for your first file changes. Go edit something and come back.
					</li>
				{/each}
			{/await}
		</ul>
	</div>

	<div class="work-in-progress-sidebar flex w-1/3 flex-col border-l border-l-zinc-700">
		<div class="recent-changes flex flex-col gap-4 border-b border-b-zinc-700 p-4">
			<h2 class="text-lg font-bold text-zinc-300">Work in Progress</h2>

			<div class="flex items-center justify-between gap-2">
				<Tooltip label={$head}>
					<div
						class="flex items-center gap-1 rounded border border-zinc-600 bg-zinc-700 py-2 px-4 text-zinc-300"
					>
						<IconGitBranch class="h-4 w-7 fill-zinc-400 stroke-none" />
						<span title={$head} class="truncate font-mono text-zinc-300">
							{$head}
						</span>
					</div>
				</Tooltip>
				<Button
					disabled={$statuses.length === 0}
					role="primary"
					href="/projects/{$project?.id}/commit"
				>
					Commit changes
				</Button>
			</div>
			{#if $statuses.length === 0}
				<div
					class="flex rounded border border-green-700 bg-green-900 p-2 align-middle text-green-400"
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
				<ul class="rounded border border-yellow-400 bg-yellow-500 p-2 font-mono text-yellow-900">
					{#each $statuses as activity}
						<li class="flex w-full gap-2">
							<span
								class:text-left={activity.staged}
								class:text-right={!activity.staged}
								class="w-[3ch] font-semibold">{activity.status.slice(0, 1).toUpperCase()}</span
							>
							<span class="truncate" use:collapsable={{ value: activity.path, separator: '/' }} />
						</li>
					{/each}
				</ul>
			{/if}
		</div>

		<div class="flex flex-auto flex-col overflow-auto ">
			<h2 class="p-4 text-lg font-bold text-zinc-300">Recent Activity</h2>

			<ul class="mx-1 flex flex-auto flex-col overflow-auto">
				{#each $recentActivity as activity}
					<li
						class="mb-2 ml-3 mr-1 flex flex-col gap-2 rounded border border-zinc-700 bg-[#2F2F33] p-3 text-zinc-400"
					>
						<div class="flex flex-row justify-between text-zinc-500">
							<span>
								{new Date(activity.timestampMs).toLocaleDateString('en-us', {
									weekday: 'short',
									year: 'numeric',
									month: 'short',
									day: 'numeric'
								})}
							</span>
							<div class="text-right font-mono">{activity.type}</div>
						</div>

						<div class="rounded-b bg-[#2F2F33] text-zinc-100">{activity.message}</div>
					</li>
				{:else}
					<li class="px-3 text-zinc-400">No activity yet.</li>
				{/each}
			</ul>
		</div>
	</div>
</div>
