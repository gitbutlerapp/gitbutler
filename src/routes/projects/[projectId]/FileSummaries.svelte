<script lang="ts">
	import { format, startOfDay } from 'date-fns';
	import type { Delta } from '$lib/api';
	import { generateBuckets } from './histogram';
	import { derived, Value } from 'svelte-loadable-store';
	import FileActivity from './FileActivity.svelte';
	import { page } from '$app/stores';
	import { Link } from '$lib/components';
	import { IconRewind, IconPlayerPlayFilled, IconLoading, IconSparkle } from '$lib/icons';
	import { collapse } from '$lib/paths';
	import type { Session } from '$lib/api';
	import { stores } from '$lib';

	export let sessions: Session[];

	$: sessionDeltas = (sessions ?? []).map(({ id, projectId }) =>
		stores.deltas({ sessionId: id, projectId })
	);

	$: deltasByDate = derived(sessionDeltas, (sessionDeltas) =>
		sessionDeltas.reduce((acc, sessionDelta) => {
			Object.entries(sessionDelta).forEach(([filepath, deltas]) => {
				const date = startOfDay(new Date(deltas[0].timestampMs)).toString();
				if (!acc[date]) acc[date] = {};
				if (!acc[date][filepath]) acc[date][filepath] = [];
				acc[date][filepath].push(...deltas);
			});
			return acc;
		}, {} as Record<string, Record<string, Delta[]>>)
	);

	$: buckets = derived(sessionDeltas, (sessionDeltas) => {
		const deltas = sessionDeltas.flatMap((deltas) => Object.values(deltas).flat());
		const timestamps = deltas.map((delta) => delta.timestampMs);
		return generateBuckets(timestamps, 18);
	});

    $: console.log($deltasByDate, $buckets)
</script>

<ul class="mr-1 flex flex-1 flex-col space-y-4 overflow-y-auto px-8 pb-8">
	{#if $deltasByDate.isLoading || $buckets.isLoading}
		<li class="flex flex-1 space-y-4 rounded-lg border border-dashed border-zinc-400">
			<div class="flex flex-1 flex-col items-center justify-center gap-4">
				<IconLoading class="h-16 w-16 animate-spin text-zinc-400 " />
				<h2 class="text-2xl font-bold text-zinc-400">Loading file changes...</h2>
			</div>
		</li>
	{:else if Value.isError($deltasByDate.value) || Value.isError($buckets.value)}
		<li class="flex flex-1 space-y-4 rounded-lg border border-dashed border-zinc-400">
			<div class="flex flex-1 flex-col items-center justify-center gap-4">
				<IconSparkle class="h-16 w-16 text-zinc-400 " />
				<h2 class="text-2xl font-bold text-zinc-400">Something went wrong</h2>
				<p class="text-zinc-400">We couldn't load your file changes. Please try again later.</p>
			</div>
		</li>
	{:else}
		{#each Object.entries($deltasByDate.value) as [ts, fileDeltas]}
			{@const date = new Date(ts)}
			<li class="card changed-day-card flex flex-col">
				<header
					class="header flex flex-row justify-between gap-2 rounded-tl rounded-tr border-b-gb-700 bg-card-active px-3 py-2"
				>
					<div class="text-zinc-300">
						{date.toLocaleDateString('en-us', {
							weekday: 'long',
							year: 'numeric',
							month: 'short',
							day: 'numeric'
						})}
					</div>
					<Link
						href="/projects/{$page.params.projectId}/player/{format(date, 'yyyy-MM-dd')}"
						role="primary"
					>
						Replay Changes
					</Link>
				</header>
				<ul class="all-files-changed flex flex-col rounded pl-3">
					{#each Object.entries(fileDeltas) as [filepath, deltas]}
						<li class="changed-file flex items-center justify-between gap-4  ">
							<a
								class="file-name max-w- flex w-full max-w-[360px] overflow-auto py-2 font-mono hover:underline"
								href="/projects/{$page.params.projectId}/player/{format(
									date,
									'yyyy-MM-dd'
								)}?file={encodeURIComponent(filepath)}"
							>
								<span class="w-full truncate">
									{collapse(filepath)}
								</span>
							</a>
							<FileActivity {deltas} buckets={$buckets.value} />
						</li>
					{/each}
				</ul>
			</li>
		{:else}
			<div
				class="replay-no-changes text-center space-y-4 border border-zinc-700 px-10 py-12 mb-6 rounded-lg h-full flex justify-around items-center text-zinc-400"
			>
				<div class="max-w-[360px] m-auto">
					<h3 class="mb-6 text-3xl font-semibold text-zinc-300">Waiting for file changes...</h3>
					<p class="mt-1">
						GitButler is now watching your project directory for file changes. As long as GitButler
						is running, changes to any text files in this directory will automatically be recorded.
					</p>
					<p class="mt-1">Since we record every change to every file, you can use GitButler to:</p>
					<ul class="space-y-4 pt-4 pb-4 mx-auto">
						<li class="flex flex-row space-x-3">
							<IconPlayerPlayFilled class="h-6 w-6 flex-none" />
							<span class="text-left">
								Replay any of your working sessions to recall what you were doing
							</span>
						</li>
						<li class="flex flex-row space-x-3">
							<IconRewind class="h-6 w-6 flex-none" />
							<span class="text-left">
								Revert to any previous working directory state or file version
							</span>
						</li>
						<li class="flex flex-row space-x-3">
							<IconSparkle class="h-6 w-6 flex-none" />
							<span class="text-left"> Get AI powered summaries of your days or sessions </span>
						</li>
					</ul>
					<p class="mt-1 text-blue-500">
						Go make a change to a file and come back to see the recording.
					</p>
				</div>
			</div>
		{/each}
	{/if}
</ul>
