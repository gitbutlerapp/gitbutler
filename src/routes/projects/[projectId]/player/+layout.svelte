<script lang="ts">
	import { page } from '$app/stores';
	import { deltas } from '$lib/api';
	import { asyncDerived, derived } from '@square/svelte-store';
	import { format } from 'date-fns';
	import type { LayoutData } from './$types';

	export let data: LayoutData;
	const { sessions } = data;

	const fileFilter = derived(page, (page) => page.url.searchParams.get('file'));

	const dates = asyncDerived([sessions, fileFilter], async ([sessions, fileFilter]) => {
		const sessionDeltas = await Promise.all(
			sessions.map((session) =>
				deltas.list({
					projectId: $page.params.projectId,
					sessionId: session.id,
					paths: fileFilter ? [fileFilter] : undefined
				})
			)
		);
		return sessions
			.filter((_, index) => Object.keys(sessionDeltas[index]).length > 0)
			.map((session) => session.meta.startTimestampMs)
			.sort((a, b) => b - a)
			.map((ts) => format(new Date(ts), 'yyyy-MM-dd'))
			.filter((date, index, self) => self.indexOf(date) === index);
	});

	const currentDate = derived(page, (page) => page.params.date);

	const today = format(new Date(), 'yyyy-MM-dd');
</script>

<div id="player-page" class="flex h-full w-full flex-col">
	{#await dates.load() then}
		{#if $dates.length === 0}
			<div class="text-center">
				<h2 class="text-2xl">I haven't seen any changes yet</h2>
				<p class="text-gray-500">Go code something!</p>
			</div>
		{:else}
			{#if $fileFilter}
				<a
					href="/projects/{$page.params.projectId}/player/{$page.params.date}/{$page.params
						.sessionId}"
					class="w-full p-2 text-left font-mono text-lg"
				>
					{$fileFilter}
				</a>
			{/if}

			<div class="flex h-full w-full flex-row gap-2 px-2">
				<ul
					id="days"
					class="scrollbar-hidden grid h-full flex-shrink-0 auto-rows-min gap-2 overflow-y-scroll py-2"
				>
					{#each $dates as date}
						{@const isToday = format(new Date(date), 'yyyy-MM-dd') === today}
						<li>
							<a
								href="/projects/{$page.params.projectId}/player/{date}{$page.url.search}"
								class:bg-card-active={date === $currentDate}
								class:text-white={date === $currentDate}
								class:border-zinc-700={date !== $currentDate}
								class:bg-card-default={date !== $currentDate}
								class="card max-h-content flex w-full flex-col items-center justify-around p-2 text-zinc-300 shadow transition duration-150 ease-out hover:ease-in"
							>
								{#if isToday}
									<div class="py-2 text-lg leading-5">Today</div>
								{:else}
									<div class="text-2xl leading-5">{new Date(date).getDate()}</div>
									<div class="leading-4">{format(new Date(date), 'MMM')}</div>
								{/if}
							</a>
						</li>
					{/each}
				</ul>

				<slot />
			</div>
		{/if}
	{/await}
</div>
