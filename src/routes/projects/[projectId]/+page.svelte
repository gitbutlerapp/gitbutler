<script lang="ts">
	import { getTime, subDays } from 'date-fns';
	import type { PageData } from './$types';
	import { IconGitBranch } from '$lib/icons';
	import { derived } from '@square/svelte-store';
	import FileSummaries from './FileSummaries.svelte';
	import { Button, Statuses, Tooltip } from '$lib/components';
	import { goto } from '$app/navigation';
	import Chat from './Chat.svelte';
	import { Loaded } from 'svelte-loadable-store';

	export let data: PageData;
	const { project, statuses, sessions, head } = data;

	$: recentSessions = derived(
		sessions,
		(item) => {
			if (Loaded.isValue(item)) {
				const lastFourDaysOfSessions = item.value?.filter(
					(result) => result.meta.startTimestampMs >= getTime(subDays(new Date(), 4))
				);
				if (lastFourDaysOfSessions?.length >= 4) return lastFourDaysOfSessions;
				return item.value
					?.slice(0, 4)
					.sort((a, b) => b.meta.startTimestampMs - a.meta.startTimestampMs);
			}
			return [];
		},
		[]
	);
</script>

<div id="project-overview" class="flex h-full w-full flex-auto">
	<div
		class="work-in-progress-sidebar side-panel flex h-full w-[424px] flex-col border-r border-light-300 dark:border-dark-600"
	>
		<div
			class="recent-changes flex flex-col gap-4 border-b border-b-light-300 p-4 dark:border-b-dark-600"
		>
			<h2 class="text-lg font-bold text-zinc-300">Work in Progress</h2>

			<div class="flex items-center justify-between gap-2">
				<Tooltip label={$head}>
					<div
						class="flex items-center gap-1 rounded border border-light-500 bg-light-700 px-4 py-2 text-dark-500 dark:border-dark-600 dark:bg-dark-700 dark:text-light-300"
					>
						<IconGitBranch class="h-4 w-7 fill-zinc-400 stroke-none" />
						<span title={$head} class="truncate font-mono text-dark-300 dark:text-light-300">
							{$head}
						</span>
					</div>
				</Tooltip>
				{#await statuses.load()}
					<Button disabled color="purple">Commit changes</Button>
				{:then}
					<Button
						disabled={Object.keys($statuses).length === 0}
						color="purple"
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

		<div class="flex flex-auto flex-col overflow-auto">
			<Chat project={$project} />
		</div>
	</div>

	<div class="main-content-container flex w-2/3 flex-auto flex-col">
		<h1 class="flex px-8 py-4 text-2xl text-dark-300 dark:text-light-300">
			<span>{$project?.title}</span>
			<span class="ml-2 text-dark-100 dark:text-light-600">Project</span>
		</h1>

		<h2 class="px-8 py-4 text-lg font-bold text-dark-300 dark:text-light-300">
			Recently changed files
		</h2>

		<FileSummaries sessions={$recentSessions} />
	</div>
</div>
