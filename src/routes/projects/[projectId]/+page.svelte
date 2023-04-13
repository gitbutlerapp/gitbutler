<script lang="ts">
	import { getTime, subDays } from 'date-fns';
	import { collapsable } from '$lib/paths';
	import type { PageData } from './$types';
	import { derived } from 'svelte/store';
	import { IconGitBranch } from '$lib/components/icons';
	import { asyncDerived } from '@square/svelte-store';
	import type { Delta } from '$lib/deltas';
	import IconRotateClockwise from '$lib/components/icons/IconRotateClockwise.svelte';
	import FileSummaries from './FileSummaries.svelte';
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

	$: fileDeltas = asyncDerived(recentSessions, async (sessions) => {
		const fileDeltas = await Promise.all(sessions.map((session) => data.getDeltas(session.id)));
		const flat = derived(fileDeltas, (fileDeltas) => {
			const merged: Record<string, Delta[]> = {};
			fileDeltas.forEach((delta) =>
				Object.entries(delta).forEach(([filepath, deltas]) => {
					if (merged[filepath]) {
						merged[filepath].push(...deltas);
					} else {
						merged[filepath] = deltas;
					}
				})
			);
			return merged;
		});
		return flat;
	});

	$: recentActivity = derived(
		[activity, recentSessions],
		([activity, recentSessions]) => {
			return activity
				.filter((a) => a.timestampMs >= (recentSessions.at(-1)?.meta.startTimestampMs ?? 0))
				.sort((a, b) => b.timestampMs - a.timestampMs);
		},
		[]
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
			{#await fileDeltas.load()}
				<li>
					<IconRotateClockwise class="animate-spin" />
				</li>
			{:then}
				<FileSummaries projectId={$project?.id} fileDeltas={$fileDeltas} />
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
				<ul
					class="rounded border border-yellow-400 bg-yellow-500 p-2 font-mono text-sm text-yellow-900"
				>
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
