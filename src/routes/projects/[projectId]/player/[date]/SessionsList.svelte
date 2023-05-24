<script lang="ts">
	import type { Delta, Session } from '$lib/api';
	import type { Readable } from '@square/svelte-store';
	import { Value, type Loadable } from 'svelte-loadable-store';
	import { derived } from 'svelte-loadable-store';
	import SessionCard from './SessionCard.svelte';

	export let sessions: (Session & {
		deltas: Readable<Loadable<Record<string, Delta[]>>>;
	})[];
	export let currentSession: Session | undefined;
	export let currentFilepath: string;

	$: visibleDeltas = derived(
		sessions.map(({ deltas }) => deltas),
		(deltas) => deltas.map((delta) => Object.fromEntries(Object.entries(delta ?? {})))
	);

	$: visibleSessions = sessions?.map((session, i) => ({
		...session,
		visible:
			!$visibleDeltas.isLoading &&
			!Value.isError($visibleDeltas.value) &&
			Object.keys($visibleDeltas.value[i]).length > 0
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
	class="mr-1 flex h-full flex-col gap-2 overflow-auto rounded-b bg-card-default pt-2 pb-2 pl-2 pr-1"
>
	{#each visibleSessions as session, i}
		{@const isCurrent = session.id === currentSession?.id}
		{#if session.visible && !$visibleDeltas.isLoading && !Value.isError($visibleDeltas.value)}
			<SessionCard {isCurrent} {session} deltas={$visibleDeltas.value[i]} {currentFilepath} />
		{/if}
	{:else}
		<div class="mt-4 text-center text-zinc-300">No activities found</div>
	{/each}
</ul>
