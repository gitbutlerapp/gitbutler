<script lang="ts">
	import { page } from '$app/stores';
	import { getSessionStore } from '$lib/stores/sessions';
	import * as deltas from '$lib/api/ipc/deltas';
	import { format } from 'date-fns';
	import { derived, Loaded } from 'svelte-loadable-store';

	const sessions = getSessionStore({ projectId: $page.params.projectId });

	$: fileFilter = $page.url.searchParams.get('file');

	const dates = derived(sessions, async (sessions) => {
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

	$: currentDate = $page.params.date;

	const today = format(new Date(), 'yyyy-MM-dd');
</script>

<div id="player-page" class="flex h-full w-full flex-col">
	{#if !$dates.isLoading && Loaded.isValue($dates)}
		{#if $dates.value.length === 0}
			<div class="text-center">
				<h2 class="text-2xl">I haven't seen any changes yet</h2>
				<p class="text-gray-500">Go code something!</p>
			</div>
		{:else}
			{#if fileFilter}
				<a
					href="/projects/{$page.params.projectId}/player/{$page.params.date}/{$page.params
						.sessionId}"
					class="w-full p-2 text-left font-mono text-lg"
				>
					{fileFilter}
				</a>
			{/if}

			<div class="flex h-full w-full flex-row gap-2 px-2">
				<ul
					id="days"
					class="scrollbar-hidden grid h-full flex-shrink-0 auto-rows-min gap-2 overflow-y-scroll py-2"
				>
					{#each $dates.value as date}
						{@const isToday = format(new Date(date), 'yyyy-MM-dd') === today}
						<li class="date-card">
							<a
								href="/projects/{$page.params.projectId}/player/{date}{$page.url.search}"
								class:bg-card-active={date === currentDate}
								class:text-white={date === currentDate}
								class:border-zinc-700={date !== currentDate}
								class:bg-card-default={date !== currentDate}
								class="card max-h-content flex w-full flex-col items-center justify-around p-2 text-zinc-300 shadow transition duration-150 ease-out hover:bg-card-active hover:ease-in"
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
	{/if}
</div>
