<script lang="ts">
	import { Week } from '$lib/week';
	import type { PageData } from './$types';
	import { WeekBlockEntry } from '$lib/components/week';
	import MdKeyboardArrowLeft from 'svelte-icons/md/MdKeyboardArrowLeft.svelte';
	import MdKeyboardArrowRight from 'svelte-icons/md/MdKeyboardArrowRight.svelte';
	import { derived } from 'svelte/store';

	export let data: PageData;
	const { project, sessions } = data;

	let week = Week.from(new Date());

	$: canNavigateForwad = week.end.getTime() < new Date().getTime();
	const formatDate = (date: Date) => {
		return new Intl.DateTimeFormat('default', {
			weekday: 'short',
			day: 'numeric',
			month: 'short'
		}).format(date);
	};

	$: sessionsInWeek = derived([sessions], ([sessions]) => {
		return sessions.filter((session) => {
			return (
				week.start <= new Date(session.meta.startTimestampMs) &&
				new Date(session.meta.startTimestampMs) <= week.end
			);
		});
	});
</script>

<div class="flex flex-col h-full select-none text-zinc-400">
	<header class="flex items-center justify-between flex-none px-8 py-1.5  border-b border-zinc-700">
		<div class="flex items-center justify-start  w-64">
			<button
				class="-ml-2 w-8 h-8 hover:text-zinc-100"
				on:click={() => (week = Week.previous(week))}
			>
				<MdKeyboardArrowLeft />
			</button>
			<div class="flex-grow text-center cursor-default grid grid-cols-7">
				<span class="col-span-3">{formatDate(Week.nThDay(week, 0))}</span>
				<span>&mdash;</span>
				<span class="col-span-3">{formatDate(Week.nThDay(week, 6))}</span>
			</div>
			<button
				class="-mr-2 w-8 h-8 hover:text-zinc-100 disabled:text-zinc-700"
				disabled={!canNavigateForwad}
				on:click={() => {
					if (canNavigateForwad) {
						week = Week.next(week);
					}
				}}
			>
				<MdKeyboardArrowRight />
			</button>
		</div>
	</header>
	<div class="isolate flex flex-col flex-auto overflow-auto">
		<div class="h-4/5 overflow-auto flex flex-col flex-none max-w-full border-b border-zinc-700">
			<!-- sticky top -->
			<div
				class="overflow-hidden sticky top-0 z-30 bg-zinc-800 border-b border-zinc-700 flex-none px-8"
			>
				<div class="grid-cols-8 divide-x divide-zinc-700 grid">
					<div class="py-4" />
					<div class="flex items-center justify-center">
						<span
							>Mon <span class="items-center justify-center font-semibold"
								>{Week.nThDay(week, 0).getDate()}</span
							></span
						>
					</div>
					<div class="flex items-center justify-center">
						<span
							>Tue <span class="items-center justify-center font-semibold"
								>{Week.nThDay(week, 1).getDate()}</span
							></span
						>
					</div>
					<div class="flex items-center justify-center">
						<span
							>Wed <span class="items-center justify-center font-semibold"
								>{Week.nThDay(week, 2).getDate()}</span
							></span
						>
					</div>
					<div class="flex items-center justify-center">
						<span
							>Thu <span class="items-center justify-center font-semibold"
								>{Week.nThDay(week, 3).getDate()}</span
							></span
						>
					</div>
					<div class="flex items-center justify-center">
						<span
							>Fri <span class="items-center justify-center font-semibold"
								>{Week.nThDay(week, 4).getDate()}</span
							></span
						>
					</div>
					<div class="flex items-center justify-center">
						<span
							>Sat <span class="items-center justify-center font-semibold"
								>{Week.nThDay(week, 5).getDate()}</span
							></span
						>
					</div>
					<div class="flex items-center justify-center">
						<span
							>Sun <span class="items-center justify-center font-semibold"
								>{Week.nThDay(week, 6).getDate()}</span
							></span
						>
					</div>
				</div>
			</div>
			<div class="flex flex-auto ">
				<div class="grid flex-auto grid-cols-1 grid-rows-1">
					<!-- hours y lines-->

					<div
						class="text-zinc-500 col-start-1 col-end-2 row-start-1 grid-rows-1 grid grid-cols-8 px-8"
					>
						<div
							class="col-start-1 col-end-2 row-start-1 grid justify-end"
							style="grid-template-rows: repeat(24, minmax(1.5rem, 1fr));"
						>
							<div class="row-end-1 h-7" />
							<div>
								<div class="z-20 -mt-2.5 -ml-14 w-14 pr-4 text-right  leading-5 text-zinc-500">
									12 AM
								</div>
							</div>
							<div />
							<div>
								<div class="z-20 -mt-2.5 -ml-14 w-14 pr-4 text-right  leading-5 text-zinc-500">
									2 AM
								</div>
							</div>
							<div />
							<div>
								<div class="z-20 -mt-2.5 -ml-14 w-14 pr-4 text-right  leading-5 text-zinc-500">
									4 AM
								</div>
							</div>
							<div />
							<div>
								<div class="z-20 -mt-2.5 -ml-14 w-14 pr-4 text-right  leading-5 text-zinc-500">
									6 AM
								</div>
							</div>
							<div />
							<div>
								<div class="z-20 -mt-2.5 -ml-14 w-14 pr-4 text-right  leading-5 text-zinc-500">
									8 AM
								</div>
							</div>
							<div />
							<div>
								<div class="z-20 -mt-2.5 -ml-14 w-14 pr-4 text-right  leading-5 text-zinc-500">
									10 AM
								</div>
							</div>
							<div />
							<div>
								<div class="z-20 -mt-2.5 -ml-14 w-14 pr-4 text-right  leading-5 text-zinc-500">
									12 PM
								</div>
							</div>
							<div />
							<div>
								<div class="z-20 -mt-2.5 -ml-14 w-14 pr-4 text-right  leading-5 text-zinc-500">
									2 PM
								</div>
							</div>
							<div />
							<div>
								<div class="z-20 -mt-2.5 -ml-14 w-14 pr-4 text-right  leading-5 text-zinc-500">
									4 PM
								</div>
							</div>
							<div />
							<div>
								<div class="z-20 -mt-2.5 -ml-14 w-14 pr-4 text-right  leading-5 text-zinc-500">
									6 PM
								</div>
							</div>
							<div />
							<div>
								<div class="z-20 -mt-2.5 -ml-14 w-14 pr-4 text-right  leading-5 text-zinc-500">
									8 PM
								</div>
							</div>
							<div />
							<div>
								<div class="z-20 -mt-2.5 -ml-14 w-14 pr-4 text-right  leading-5 text-zinc-500">
									10 PM
								</div>
							</div>
							<div />
						</div>
					</div>

					<div
						class="text-zinc-500 col-start-1 col-end-2 row-start-1 grid-rows-1 grid grid-cols-8 ml-8 "
					>
						<div
							class="col-start-2 col-end-9 row-start-1 grid divide-y divide-zinc-700/20"
							style="grid-template-rows: repeat(24, minmax(1.5rem, 1fr));"
						>
							<div class="row-end-1 h-7" />
							<div>
								<div
									class="h-7 left-0 z-20 -mt-2.5 -ml-14 w-14 pr-2 text-right  leading-5 text-zinc-500"
								/>
							</div>
							<div />
							<div>
								<div
									class="h-7 left-0 z-20 -mt-2.5 -ml-14 w-14 pr-2 text-right  leading-5 text-zinc-500"
								/>
							</div>
							<div />
							<div>
								<div
									class="h-7 left-0 z-20 -mt-2.5 -ml-14 w-14 pr-2 text-right  leading-5 text-zinc-500"
								/>
							</div>
							<div />
							<div>
								<div
									class="h-7 left-0 z-20 -mt-2.5 -ml-14 w-14 pr-2 text-right  leading-5 text-zinc-500"
								/>
							</div>
							<div />
							<div>
								<div
									class="h-7 left-0 z-20 -mt-2.5 -ml-14 w-14 pr-2 text-right  leading-5 text-zinc-500"
								/>
							</div>
							<div />
							<div>
								<div
									class="h-7 left-0 z-20 -mt-2.5 -ml-14 w-14 pr-2 text-right  leading-5 text-zinc-500"
								/>
							</div>
							<div />
							<div>
								<div
									class="h-7 left-0 z-20 -mt-2.5 -ml-14 w-14 pr-2 text-right  leading-5 text-zinc-500"
								/>
							</div>
							<div />
							<div>
								<div
									class="h-7 left-0 z-20 -mt-2.5 -ml-14 w-14 pr-2 text-right  leading-5 text-zinc-500"
								/>
							</div>
							<div />
							<div>
								<div
									class="h-7 left-0 z-20 -mt-2.5 -ml-14 w-14 pr-2 text-right  leading-5 text-zinc-500"
								/>
							</div>
							<div />
							<div>
								<div
									class="h-7 left-0 z-20 -mt-2.5 -ml-14 w-14 pr-2 text-right  leading-5 text-zinc-500"
								/>
							</div>
							<div />
							<div>
								<div
									class="h-7 left-0 z-20 -mt-2.5 -ml-14 w-14 pr-2 text-right  leading-5 text-zinc-500"
								/>
							</div>
							<div />
							<div>
								<div
									class="h-7 left-0 z-20 -mt-2.5 -ml-14 w-14 pr-2 text-right  leading-5 text-zinc-500"
								/>
							</div>
						</div>
						<div />
					</div>

					<!-- day x lines -->
					<div
						class="col-start-1 col-end-2 row-start-1 grid-rows-1 divide-x divide-zinc-700/50 grid grid-cols-8 px-8"
					>
						<div />
						<div />
						<div />
						<div />
						<div />
						<div />
						<div />
						<div />
					</div>

					<!-- actual entries -->
					<ol
						class="col-start-1 col-end-2 row-start-1 grid grid-cols-8 px-8"
						style="grid-template-rows: 1.75rem repeat(96, minmax(0px, 1fr)) auto;"
					>
						{#each $sessionsInWeek as session}
							<WeekBlockEntry
								startTime={new Date(session.meta.startTimestampMs)}
								endTime={new Date(session.meta.startTimestampMs)}
								label={session.meta.branch}
								href="/projects/{$project?.id}/sessions/{session.id}/"
							/>
						{/each}
					</ol>
				</div>
			</div>
		</div>
	</div>
</div>
