<script lang="ts">
	import type { PageData } from './$types';
	import { derived, writable } from 'svelte/store';
	import { listFiles } from '$lib/sessions';
	import { type Delta, list as listDeltas } from '$lib/deltas';
	import { CodeViewer } from '$lib/components';
	import {
		IconPlayerPauseFilled,
		IconPlayerPlayFilled,
		IconPlayerTrackNextFilled,
		IconPlayerTrackPrevFilled
	} from '@tabler/icons-svelte';

	export let data: PageData;

	const { sessions } = data;

	const currentTimestamp = writable(new Date().getTime());

	// session at the current cursor
	const currentSession = derived([currentTimestamp, sessions], ([currentTimestamp, sessions]) => {
		const ss = sessions
			.sort((a, b) => b.meta.startTimestampMs - a.meta.startTimestampMs)
			.filter((s) => s.meta.startTimestampMs <= currentTimestamp);
		if (ss.length === 0) sessions[0];
		return ss[0];
	});

	const previousSession = derived([currentSession, sessions], ([$currentSession, $sessions]) => {
		const index = $sessions.findIndex((session) => session.id === $currentSession.id);
		if (index === -1) throw new Error('current session not found in sessions');
		const nextIndex = index + 1;
		if (nextIndex >= $sessions.length) return null;
		return $sessions[nextIndex];
	});

	const nextSession = derived([currentSession, sessions], ([$currentSession, $sessions]) => {
		const index = $sessions.findIndex((session) => session.id === $currentSession.id);
		if (index === -1) throw new Error('current session not found in sessions');
		const previousIndex = index - 1;
		if (previousIndex < 0) return null;
		return $sessions[previousIndex];
	});

	// currentSessionFileByFilepath is a map from filepath to the file contents at the beginning of the session
	const currentSessionFileByFilepath = writable<Record<string, string>>({});
	$: listFiles({ projectId: data.projectId, sessionId: $currentSession.id }).then(
		currentSessionFileByFilepath.set
	);

	// currentSessionDeltasByFilepath is a map from filepath to the deltas that were applied to the file during the session
	const currentSessionDeltasByFilepath = writable<Record<string, Delta[]>>({});
	$: listDeltas({ projectId: data.projectId, sessionId: $currentSession.id }).then(
		currentSessionDeltasByFilepath.set
	);

	// currentSessionTimestamps is a list of all timestamps that were recorded during the session in descending order
	const currentSessionTimestamps = derived(
		[currentSessionDeltasByFilepath],
		([currentSessionDeltasByFilepath]) => {
			const deltas = Object.values(currentSessionDeltasByFilepath);
			const timestamps = deltas.flatMap((deltas) => deltas.map((delta) => delta.timestampMs));
			return timestamps.sort((a, b) => b - a);
		}
	);

	const nextDeltaTimestamp = derived(
		[currentTimestamp, currentSessionTimestamps],
		([currentTimestamp, currentSessionTimestamps]) => {
			return (
				currentSessionTimestamps.filter((timestamp) => timestamp > currentTimestamp).at(-1) ?? null
			);
		},
		null
	);

	const previousDeltaTimestamp = derived(
		[currentTimestamp, currentSessionTimestamps],
		([currentTimestamp, currentSessionTimestamps]) => {
			return (
				currentSessionTimestamps.filter((timestamp) => timestamp < currentTimestamp).at(0) ?? null
			);
		},
		null
	);

	const previousTimestamp = derived(
		[previousDeltaTimestamp, previousSession],
		([previousDeltaTimestamp, previousSession]) => {
			return previousDeltaTimestamp ?? previousSession?.meta.lastTimestampMs ?? null;
		}
	);
	const nextTimestamp = derived(
		[nextDeltaTimestamp, nextSession],
		([nextDeltaTimestamp, nextSession]) => {
			return nextDeltaTimestamp ?? nextSession?.meta.startTimestampMs ?? null;
		}
	);

	const currentFilepath = derived(
		[currentSessionDeltasByFilepath, currentTimestamp],
		([currentSessionDeltasByFilepath, currentTimestamp]) =>
			Object.entries(currentSessionDeltasByFilepath)
				.map(
					([filepath, deltas]) =>
						[filepath, deltas.filter((delta) => delta.timestampMs <= currentTimestamp)] as [
							string,
							Delta[]
						]
				)
				.filter(([_, deltas]) => deltas.length > 0)
				.map(([filepath, deltas]) => [filepath, deltas.at(-1)!.timestampMs] as [string, number])
				.sort((a, b) => b[1] - a[1])
				.at(0)?.[0] ?? null
	);

	const currentDeltas = derived(
		[currentSessionDeltasByFilepath, currentFilepath, currentTimestamp],
		([currentSessionDeltasByFilepath, currentFilepath, currentTimestamp]) =>
			currentFilepath
				? (currentSessionDeltasByFilepath[currentFilepath] ?? []).filter(
						(delta) => delta.timestampMs <= currentTimestamp
				  )
				: null
	);

	const currentDoc = derived(
		[currentSessionFileByFilepath, currentFilepath],
		([currentSessionFileByFilepath, currentFilepath]) =>
			currentFilepath ? currentSessionFileByFilepath[currentFilepath] ?? '' : null
	);

	let interval: ReturnType<typeof setInterval> | undefined;
	let direction: -1 | 1 = 1;
	let speed = 1;
	let oneSecond = 1000;

	const stop = () => {
		clearInterval(interval);
		interval = undefined;
	};
	const play = () => start({ direction, speed });

	const start = (params: { direction: 1 | -1; speed: number }) => {
		if (interval) stop();
		interval = setInterval(() => {
			currentTimestamp.update((ts) => ts + oneSecond * params.direction);
		}, oneSecond / params.speed);
	};

	const backward = () => {
		direction === -1 ? (speed = speed * 2) : ((direction = -1), (speed = 1));
		start({ direction, speed });
	};

	const forward = () => {
		direction === 1 ? (speed = speed * 2) : ((direction = 1), (speed = 1));
		start({ direction, speed });
	};
	$: console.log(interval);
</script>

<div class="flex h-full flex-col gap-2 px-12">
	<div>
		<div>current session id {$currentSession.id}</div>
		<div>current session hash {$currentSession.hash}</div>
		<div>current filepath {$currentFilepath}</div>
		<div>current deltas.length {$currentDeltas?.length}</div>
		<div>current doc.length {$currentDoc?.length}</div>
		<div>previous timestamp {new Date($previousTimestamp ?? 0).toTimeString()}</div>
		<div>current timestamp {new Date($currentTimestamp).toTimeString()}</div>
		<div>next timestamp {new Date($nextTimestamp ?? 0).toTimeString()}</div>
	</div>

	<div class="flex-auto overflow-auto">
		{#key $currentSession.id}
			{#if $currentDoc !== null && $currentDeltas !== null && $currentFilepath !== null}
				<CodeViewer doc={$currentDoc} filepath={$currentFilepath} deltas={$currentDeltas} />
			{/if}
		{/key}
	</div>

	<div class="mx-auto flex items-center gap-2">
		<button on:click={backward}><IconPlayerTrackPrevFilled class="h-6 w-6" /></button>
		{#if interval}
			<button on:click={stop}><IconPlayerPauseFilled class="h-6 w-6" /></button>
		{:else}
			<button on:click={play}><IconPlayerPlayFilled class="h-6 w-6" /></button>
		{/if}
		<button on:click={forward}><IconPlayerTrackNextFilled class="h-6 w-6" /></button>
	</div>
</div>
