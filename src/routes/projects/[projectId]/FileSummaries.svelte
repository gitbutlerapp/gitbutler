<script lang="ts">
	import { format, startOfDay } from 'date-fns';
	import { deltas, type Delta } from '$lib/api';
	import { derived } from '@square/svelte-store';
	import FileActivity from './FileActivity.svelte';
	import { page } from '$app/stores';
	import { Link } from '$lib/components';
	import { IconRewind, IconPlayerPlayFilled, IconLoading, IconSparkle } from '$lib/icons';
	import { bucketByTimestamp } from './histogram';
	import { collapse } from '$lib/paths';
	import type { Session } from '$lib/api';

	export let sessions: Session[];

	$: sessionDeltas = (sessions ?? []).map(({ id, projectId }) =>
		deltas.Deltas({ sessionId: id, projectId })
	);

	$: deltasByDate = derived(sessionDeltas, (sessionDeltas) =>
		sessionDeltas?.reduce((acc, sessionDelta) => {
			Object.entries(sessionDelta ?? {}).forEach(([filepath, deltas]) => {
				const date = startOfDay(new Date(deltas[0].timestampMs)).toString();
				if (!acc[date]) acc[date] = {};
				if (!acc[date][filepath]) acc[date][filepath] = [];
				acc[date][filepath].push(...deltas);
			});
			return acc;
		}, {} as Record<string, Record<string, Delta[]>>)
	);

	const getLargestBucket = (fileDeltas: Record<string, Delta[]>): number =>
		Math.max(
			...Object.entries(fileDeltas).map((entry) => {
				const deltas = entry[1];
				return Math.max(
					...bucketByTimestamp(
						deltas.map((delta) => delta.timestampMs),
						18
					).map((bucket) => bucket.length)
				);
			})
		);
</script>

<ul class="flex flex-1 flex-col space-y-4 overflow-y-auto pr-1">
	{#await deltasByDate.load()}
		<li class="flex flex-1 space-y-4 rounded-lg border border-dashed border-zinc-400">
			<div class="flex flex-1 flex-col items-center justify-center gap-4">
				<IconLoading class="h-16 w-16 animate-spin text-zinc-400 " />
				<h2 class="text-2xl font-bold text-zinc-400">Loading file changes...</h2>
			</div>
		</li>
	{:then}
		{#each Object.entries($deltasByDate) as [ts, fileDeltas]}
			{@const largestBucketSize = getLargestBucket(fileDeltas)}
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
							<FileActivity {deltas} {largestBucketSize} />
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
	{/await}
</ul>
