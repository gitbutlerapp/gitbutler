<script lang="ts">
	import { themeIcons } from 'seti-icons';
	import type { Session } from '$lib/sessions';
	import { toHumanReadableTime, toHumanReadableDate } from '$lib/time';
	import { toHumanBranchName } from '$lib/branch';
	import TimelineDaySessionActivities from './TimelineDaySessionActivities.svelte';
	import { list } from '$lib/deltas';
	export let session: Session;
	export let projectId: string;

	const getIcon = themeIcons({
		blue: '#268bd2',
		grey: '#657b83',
		'grey-light': '#839496',
		green: '#859900',
		orange: '#cb4b16',
		pink: '#d33682',
		purple: '#6c71c4',
		red: '#dc322f',
		white: '#fdf6e3',
		yellow: '#b58900',
		ignore: '#586e75'
	});

	function pathToName(path: string) {
		return path.split('/').slice(-1)[0];
	}
	function pathToIconSvg(path: string) {
		let name: string = pathToName(path);
		let { svg } = getIcon(name);
		return svg;
	}
	const colorFromBranchName = (branchName: string | undefined) => {
		branchName = branchName || 'master';
		const colors = [
			'bg-red-500 border-red-700',
			'bg-green-500 border-green-700',
			'bg-blue-500 border-blue-700',
			'bg-yellow-500 border-yellow-700',
			'bg-purple-500 border-purple-700',
			'bg-pink-500 border-pink-700',
			'bg-indigo-500 border-indigo-700',
			'bg-orange-500 border-orange-700'
		];
		const hash = branchName.split('').reduce((acc, char) => {
			return acc + char.charCodeAt(0);
		}, 0);
		return colors[hash % colors.length];
	};
</script>

<div class="flex flex-col space-y-2">
	<span class="relative inline-flex">
		<a
			id="block"
			class="inline-flex flex-grow items-center truncate transition ease-in-out duration-150 border px-4 py-2 text-slate-50 rounded-lg {colorFromBranchName(
				session.meta.branch
			)}"
			title={session.meta.branch}
			href="/projects/{projectId}/sessions/{session.id}/"
		>
			{toHumanBranchName(session.meta.branch)}
		</a>
		{#if !session.hash}
			<span class="flex absolute h-3 w-3 top-0 right-0 -mt-1 -mr-1" title="Current session">
				<span
					class="animate-ping absolute inline-flex h-full w-full rounded-full bg-orange-200 opacity-75"
				/>
				<span
					class="relative inline-flex rounded-full h-3 w-3 bg-zinc-200 border border-orange-200"
				/>
			</span>
		{/if}
	</span>
	<div id="activities">
		<div class="my-2 mx-1">
			<TimelineDaySessionActivities
				activities={session.activity}
				sessionStart={session.meta.startTimestampMs}
				sessionEnd={session.meta.lastTimestampMs}
			/>
		</div>
	</div>
	<div id="time-range" class="">
		{toHumanReadableDate(session.meta.startTimestampMs)},
		{toHumanReadableTime(session.meta.startTimestampMs)}
		<div class=" text-zinc-600">
			{Math.round((session.meta.lastTimestampMs - session.meta.startTimestampMs) / 60 / 1000)} min
		</div>
	</div>
	<div id="files">
		{#await list({ projectId: projectId, sessionId: session.id }) then deltas}
			{#each Object.keys(deltas) as delta}
				<div class="flex flex-row w-32 items-center">
					<div class="w-6 h-6 text-white fill-blue-400">
						{@html pathToIconSvg(delta)}
					</div>
					<div class="text-white w-24 truncate">
						{pathToName(delta)}
					</div>
				</div>
			{/each}
		{/await}
	</div>
</div>
