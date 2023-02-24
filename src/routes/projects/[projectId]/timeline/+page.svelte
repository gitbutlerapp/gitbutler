<script lang="ts">
	import { themeIcons } from 'seti-icons';
	import type { PageData } from './$types';
	import { derived } from 'svelte/store';
	import type { Session } from '$lib/sessions';
	import { startOfDay } from 'date-fns';
	import { list as listDeltas } from '$lib/deltas';

	export let data: PageData;
	const { project, sessions } = data;

	const formatDate = (date: Date) => {
		return new Intl.DateTimeFormat('default', {
			weekday: 'short',
			day: 'numeric',
			month: 'short'
		}).format(date);
	};

	const formatTime = (date: Date) => {
		return new Intl.DateTimeFormat('default', {
			hour: 'numeric',
			minute: 'numeric'
		}).format(date);
	};

	function pathToName(path: string) {
		return path.split('/').slice(-1)[0];
	}

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

	function pathToIconSvg(path: string) {
		let name: string = pathToName(path);
		let { svg } = getIcon(name);
		return svg;
	}

	$: dateSessions = derived([sessions], ([sessions]) => {
		const dateSessions: Record<number, Session[]> = {};
		sessions.forEach((session) => {
			const date = startOfDay(new Date(session.meta.startTimestampMs));
			if (dateSessions[date.getTime()]) {
				dateSessions[date.getTime()]?.push(session);
			} else {
				dateSessions[date.getTime()] = [session];
			}
		});
		// TODO: maybe sort?
		return dateSessions;
	});
</script>

<div class="flex">
	<div class="m-6 overflow-x-hidden w-full">
		All sessions
		<div class="flex flex-row border overflow-x-auto space-x-12 px-4 py-12">
			{#each Object.entries($dateSessions) as [dateMilliseconds, sessions]}
				<!-- Day -->
				<div class="bg-zinc-600 py-1 min-w-full overflow-hidden">
					<div>{formatDate(new Date(+dateMilliseconds))}</div>
					<div class="flex space-x-2 ">
						{#each sessions as session}
							<!-- Session -->
							<div>
								<div class="text-sm rounded borded bg-orange-500 text-zinc-200">
									{formatTime(new Date(session.meta.startTimestampMs))}
									-
									{formatTime(new Date(session.meta.lastTimestampMs))}
								</div>
								<div title="Session duration">
									{Math.round(
										(session.meta.lastTimestampMs - session.meta.startTimestampMs) / 1000 / 60
									)} min
								</div>
								<div title="Session files">
									{#await listDeltas( { projectId: $project?.id, sessionId: session.id } ) then deltas}
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
						{/each}
					</div>
				</div>
			{/each}
		</div>
	</div>
</div>
