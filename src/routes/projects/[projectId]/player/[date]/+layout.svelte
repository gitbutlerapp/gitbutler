<script lang="ts">
	import { page } from '$app/stores';
	import { format } from 'date-fns';
	import { derived } from 'svelte/store';
	import type { LayoutData } from './$types';

	export let data: LayoutData;
	const { sessions, projectId } = data;

	const dates = derived(sessions, (sessions) =>
		sessions
			.map((session) => session.meta.startTimestampMs)
			.sort((a, b) => b - a)
			.map((ts) => format(new Date(ts), 'yyyy-MM-dd'))
			.filter((date, index, self) => self.indexOf(date) === index)
	);

	const currentDate = derived(page, (page) => page.params.date);

	const fileFilter = derived(page, (page) => page.url.searchParams.get('file'));
</script>

{#if $sessions.length === 0}
	<div class="text-center">
		<h2 class="text-xl">I haven't seen any changes yet</h2>
		<p class="text-gray-500">Go code something!</p>
	</div>
{:else}
	{#if $fileFilter}
		<a
			href="/projects/{$page.params.projectId}/player/{$page.params.date}/{$page.params.sessionId}"
			class="w-full p-2 text-left font-mono text-lg"
		>
			{$fileFilter}
		</a>
	{/if}

	<div class="flex h-full w-full flex-row gap-2 px-2">
		<ul
			id="days"
			class="scrollbar-hidden flex h-full flex-shrink-0 flex-col gap-2 overflow-y-scroll py-2 pr-[1px]"
		>
			{#each $dates as date}
				<li class="w-full">
					<a
						href="/projects/{projectId}/player/{date}{$page.url.search}"
						class:bg-gb-800={date === $currentDate}
						class:text-white={date === $currentDate}
						class:border-gb-700={date !== $currentDate}
						class:bg-gb-900={date !== $currentDate}
						class="flex w-full flex-col items-center rounded border border-[0.5px] p-2 text-zinc-300 shadow transition duration-150 ease-out hover:bg-gb-800 hover:ease-in"
					>
						<div class="text-xl leading-5">{new Date(date).getDate()}</div>
						<div class="leading-4">{format(new Date(date), 'MMM')}</div>
					</a>
				</li>
			{/each}
		</ul>

		<slot />
	</div>
{/if}
