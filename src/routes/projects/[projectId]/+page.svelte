<script lang="ts">
	import { getTime, subDays } from 'date-fns';
	import type { PageData } from './$types';
	import { IconGitBranch, IconLoading } from '$lib/icons';
	import { derived } from '@square/svelte-store';
	import FileSummaries from './FileSummaries.svelte';
	import { Button, Statuses, Tooltip } from '$lib/components';
	import { goto } from '$app/navigation';
	import Chat from './Chat.svelte';

	export let data: PageData;
	$: activity = derived(data.activity, (activity) => activity);
	$: project = derived(data.project, (project) => project);
	$: statuses = derived(data.statuses, (statuses) => statuses);
	$: sessions = derived(data.sessions, (sessions) => sessions);
	$: head = derived(data.head, (head) => head);

	$: recentSessions = derived(
		sessions,
		(sessions) => {
			const lastFourDaysOfSessions = sessions?.filter(
				(session) => session.meta.startTimestampMs >= getTime(subDays(new Date(), 4))
			);
			if (lastFourDaysOfSessions?.length >= 4) return lastFourDaysOfSessions;
			return sessions
				?.slice(0, 4)
				.sort((a, b) => b.meta.startTimestampMs - a.meta.startTimestampMs);
		},
		[]
	);

	$: recentActivity = derived(
		[activity, recentSessions],
		([activity, recentSessions]) =>
			recentSessions?.length
				? activity
						?.filter((a) => a.timestampMs >= (recentSessions?.at(-1)?.meta.startTimestampMs ?? 0))
						.sort((a, b) => b.timestampMs - a.timestampMs)
						.slice(0, 100)
				: [],
		[]
	);
</script>

<div id="project-overview" class="flex h-full w-full flex-auto">
	<div
		class="work-in-progress-sidebar side-panel flex h-full w-[424px] flex-col border-r  border-light-300 dark:border-dark-600"
	>
		<div
			class="recent-changes flex flex-col gap-4 border-b border-b-light-300 p-4 dark:border-b-dark-600 "
		>
			<h2 class="text-lg font-bold text-zinc-300">Work in Progress</h2>

			<div class="flex items-center justify-between gap-2">
				<Tooltip label={$head}>
					<div
						class="flex items-center gap-1 rounded border border-light-500 bg-light-700 py-2 px-4 text-dark-500 dark:border-dark-600 dark:bg-dark-700 dark:text-light-300"
					>
						<IconGitBranch class="h-4 w-7 fill-zinc-400 stroke-none" />
						<span title={$head} class="truncate font-mono text-dark-300 dark:text-light-300">
							{$head}
						</span>
					</div>
				</Tooltip>
				{#await statuses.load()}
					<Button disabled color="primary">Commit changes</Button>
				{:then}
					<Button
						disabled={Object.keys($statuses).length === 0}
						color="primary"
						on:click={() => goto(`/projects/${$project?.id}/commit`)}
					>
						Commit changes
					</Button>
				{/await}
			</div>
			{#await statuses.load() then}
				<Statuses statuses={$statuses} />
			{/await}
		</div>

		<div class="flex flex-auto flex-col overflow-auto ">
			<Chat project={$project} />
		</div>
	</div>

	<div class="main-content-container flex w-2/3 flex-auto flex-col">
		<h1 class="flex py-4 px-8 text-2xl text-dark-300 dark:text-light-300">
			<span>{$project?.title}</span>
			<span class="ml-2 text-dark-100 dark:text-light-600">Project</span>
		</h1>

		<h2 class="py-4 px-8 text-lg font-bold text-dark-300 dark:text-light-300">
			Recently changed files
		</h2>

		<FileSummaries sessions={$recentSessions} />
	</div>
</div>
