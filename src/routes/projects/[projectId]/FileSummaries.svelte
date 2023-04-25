<script lang="ts">
	import { format, startOfDay } from 'date-fns';
	import type { Delta } from '$lib/api';
	import { derived, type Readable } from 'svelte/store';
	import { collapsable } from '$lib/paths';
	import FileActivity from './FileActivity.svelte';
	import { Button } from '$lib/components';
	import { bucketByTimestamp } from './histogram';

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

	const getLargestBucket = (fileDeltas: Record<string, Delta[]>): number => {
		return Math.max(
			...Object.entries(fileDeltas).map(([filepath, deltas]) => {
				return Math.max(
					...bucketByTimestamp(
						deltas.map((delta) => delta.timestampMs),
						18
					).map((bucket) => bucket.length)
				);
			})
		);
	};
</script>

{#each $fileDeltasByDate as [date, fileDeltas, largestBucketSize]}
	<li
		class="card changed-day-card flex flex-col rounded border-[0.5px] border-gb-700 bg-card-default"
	>
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
			<Button
				href="/projects/{projectId}/player/{format(date, 'yyyy-MM-dd')}"
				filled={false}
				role="primary"
			>
				Replay Changes
			</Button>
		</header>
		<ul class="all-files-changed flex flex-col rounded pl-3">
			{#each Object.entries(fileDeltas) as [filepath, deltas]}
				<li class="changed-file flex items-center justify-between gap-4  ">
					<a
						class="file-name flex w-[50%] overflow-auto py-2 font-mono hover:underline"
						href="/projects/{projectId}/player/{format(
							date,
							'yyyy-MM-dd'
						)}?file={encodeURIComponent(filepath)}"
					>
						<span class="w-full truncate" use:collapsable={{ value: filepath, separator: '/' }} />
					</a>
					<FileActivity {deltas} {largestBucketSize} />
				</li>
			{/each}
		</ul>
	</li>
{:else}
	<li class="text-zinc-400">
		Waiting for your first file changes. Go edit something and come back.
	</li>
{/each}
