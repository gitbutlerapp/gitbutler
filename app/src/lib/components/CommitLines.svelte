<script lang="ts">
	import LocalLine from './LocalLine.svelte';
	import RemoteLine from './RemoteLine.svelte';
	import ShadowLine from './ShadowLine.svelte';
	import type { Commit, RemoteCommit } from '$lib/vbranches/types';

	export let hasShadowColumn = false;
	export let hasLocalColumn = false;
	export let localCommit: Commit | undefined = undefined;
	export let remoteCommit: RemoteCommit | undefined = undefined;
	export let first = false;
	export let localLine = false;
	export let localRoot = false;
	export let remoteLine = false;
	export let upstreamLine = false;
	export let shadowLine = false;
	export let base = false;
</script>

<div class="lines">
	{#if hasShadowColumn}
		<ShadowLine
			line={shadowLine}
			dashed={base}
			{upstreamLine}
			{remoteCommit}
			localCommit={localCommit?.relatedTo ? localCommit : undefined}
			{first}
			short={(!!localCommit && !localCommit?.children?.[0]?.relatedTo) ||
				(!!remoteCommit && !remoteCommit?.children?.[0])}
		/>
	{/if}
	<RemoteLine
		commit={localCommit?.status == 'remote' ? localCommit : undefined}
		line={localCommit?.status == 'remote' || remoteLine}
		root={localRoot ||
			(localCommit?.status == 'remote' && localCommit?.children?.[0]?.status == 'local')}
		remoteCommit={!hasShadowColumn ? remoteCommit : undefined}
		shadowCommit={!hasShadowColumn &&
		localCommit?.relatedTo &&
		localCommit.relatedTo.id != localCommit.id
			? localCommit.relatedTo
			: undefined}
		upstreamLine={upstreamLine && !hasShadowColumn}
		{first}
		short={(!!localCommit && !localCommit?.children?.[0] && !upstreamLine) ||
			(!!remoteCommit && !remoteCommit?.children?.[0])}
		{base}
	/>

	{#if hasLocalColumn}
		<LocalLine
			isEmpty={base}
			commit={localCommit?.status == 'local' ? localCommit : undefined}
			dashed={localLine}
			{first}
		/>
	{/if}
</div>

<style lang="postcss">
	.lines {
		display: flex;
		align-items: stretch;
		min-height: var(--size-12);
		padding-left: var(--size-8);
		/* margin-top: -1px; */
	}
</style>
