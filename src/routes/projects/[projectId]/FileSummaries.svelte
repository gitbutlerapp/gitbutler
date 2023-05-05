<script lang="ts">
	import { format, startOfDay } from 'date-fns';
	import type { Delta } from '$lib/api';
	import { derived, type Readable } from '@square/svelte-store';
	import FileActivity from './FileActivity.svelte';
	import { Link } from '$lib/components';
	import { bucketByTimestamp } from './histogram';
	import { collapse } from '$lib/paths';
	import IconRewind from '$lib/components/icons/IconRewind.svelte';
	import IconSparkle from '$lib/components/icons/IconSparkle.svelte';
	import IconPlayerPlayFilled from '$lib/components/icons/IconPlayerPlayFilled.svelte';

	export let projectId: string;
	export let fileDeltas: Readable<Record<string, Delta[]>>;

	$: fileDeltasByDate = derived(fileDeltas, (fileDeltas) => {
		const merged: Record<string, Record<string, Delta[]>> = {};
		Object.entries(fileDeltas).forEach(([filepath, deltas]) => {
			deltas.forEach((delta) => {
				const date = new Date(delta.timestampMs).toISOString().split('T')[0];
				if (merged[date]) {
					if (merged[date][filepath]) {
						merged[date][filepath].push(delta);
					} else {
						merged[date][filepath] = [delta];
					}
				} else {
					merged[date] = { [filepath]: [delta] };
				}
			});
		});

		return Object.entries(merged)
			.reduce((acc, [date, fileDeltas]) => {
				const d = startOfDay(new Date(date));
				acc.push([d, fileDeltas, getLargestBucket(fileDeltas)]);
				return acc;
			}, [] as [Date, Record<string, Delta[]>, number][])
			.sort((a, b) => b[0].getTime() - a[0].getTime());
	});

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

{#each $fileDeltasByDate as [date, fileDeltas, largestBucketSize]}
	<li class="card changed-day-card flex flex-col">
		<header
			class="header flex flex-row justify-between gap-2 rounded-tl rounded-tr border-b-gb-700 bg-card-active px-3 py-2"
		>
			<div class=" text-zinc-300 ">
				{date.toLocaleDateString('en-us', {
					weekday: 'long',
					year: 'numeric',
					month: 'short',
					day: 'numeric'
				})}
			</div>
			<Link href="/projects/{projectId}/player/{format(date, 'yyyy-MM-dd')}" role="primary">
				Replay Changes
			</Link>
		</header>
		<ul class="all-files-changed flex flex-col rounded pl-3">
			{#each Object.entries(fileDeltas) as [filepath, deltas]}
				<li class="changed-file flex items-center justify-between gap-4  ">
					<a
						class="file-name max-w- flex w-full max-w-[360px] overflow-auto py-2 font-mono hover:underline"
						href="/projects/{projectId}/player/{format(
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
	<div class="space-y-4 border border-zinc-400 border-dashed px-10 py-12 rounded-lg">
		<h3 class="mt-2 text-lg font-semibold text-blue-500">Waiting for file changes...</h3>
		<p class="mt-1 text-gray-400">
			GitButler is now watching your project directory for file changes. As long as GitButler is
			running, changes to any text files in this directory will automatically be recorded.
		</p>
		<p class="mt-1 text-gray-400">
			Since we record every change to every file, you can use GitButler to:
		</p>
		<ul class="space-y-4 pt-2 pb-4 text-zinc-400">
			<li class="flex flex-row space-x-3">
				<IconPlayerPlayFilled class="h-6 w-6 flex-none" />
				<span class="text-zinc-200"
					>Replay any of your working sessions to recall what you were doing</span
				>
			</li>
			<li class="flex flex-row space-x-3">
				<IconRewind class="h-6 w-6 flex-none" />
				<span class="text-zinc-200"
					>Revert to any previous working directory state or file version
				</span>
			</li>
			<li class="flex flex-row space-x-3">
				<IconSparkle class="h-6 w-6 flex-none" />
				<span class="text-zinc-200">Get AI powered summaries of your days or sessions </span>
			</li>
		</ul>
		<p class="mt-1 text-blue-500">Go make a change to a file and come back to see the recording.</p>
	</div>
{/each}
