<script lang="ts">
	import type { Session } from '$lib/api/ipc/sessions';
	import type { Delta } from '$lib/api/ipc/deltas';
	import { asyncDerived, type Readable } from '@square/svelte-store';
	import SessionCard from './SessionCard.svelte';

	export let sessions: (Session & {
		deltas: Readable<Record<string, Delta[]>>;
		files: Readable<Record<string, string>>;
	})[];
	export let currentSession: Session | undefined;
	export let currentFilepath: string;

	$: visibleDeltas = asyncDerived(
		sessions.map(({ deltas }) => deltas),
		async (deltas) => deltas.map((delta) => Object.fromEntries(Object.entries(delta ?? {})))
	);

	$: visibleFiles = asyncDerived(
		sessions.map(({ files }) => files),
		async (files) => files.map((file) => Object.fromEntries(Object.entries(file ?? {})))
	);

	$: visibleSessions = sessions?.map((session, i) => ({
		...session,
		visible: Object.keys($visibleDeltas[i]).length > 0
	}));
</script>

<header
	class="card-header flex flex-row justify-between rounded-t border-b-[1px] border-b-divider bg-card-active px-3 py-2 leading-[21px]"
>
	<div class="relative flex gap-2">
		<div class="relative bottom-[1px] h-4 w-4 text-sm">🧰</div>
		<div>Working History</div>
		<div class="text-zinc-400">
			{visibleSessions.filter(({ visible }) => visible).length}
		</div>
	</div>
</header>

<ul
	class="mr-1 flex h-full flex-col gap-2 overflow-auto rounded-b bg-card-default pb-2 pl-2 pr-1 pt-2"
>
	{#each visibleSessions as session, i}
		{@const isCurrent = session.id === currentSession?.id}
		{#if session.visible && $visibleDeltas && $visibleFiles}
			<SessionCard
				{isCurrent}
				{session}
				deltas={$visibleDeltas[i]}
				files={$visibleFiles[i]}
				{currentFilepath}
			/>
		{/if}
	{:else}
		<div class="mt-4 text-center text-zinc-300">No activities found</div>
	{/each}
</ul>
