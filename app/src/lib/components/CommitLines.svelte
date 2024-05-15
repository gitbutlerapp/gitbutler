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
	export let remoteLine = false;
	export let upstreamLine = false;
	export let shadowLine = false;
	export let base = false;
</script>

<div class="lines" class:base>
	{#if hasShadowColumn}
		<ShadowLine
			line={shadowLine}
			dashed={base}
			{upstreamLine}
			{remoteCommit}
			{localCommit}
			{first}
		/>
	{/if}
	<RemoteLine
		commit={localCommit?.status == 'remote' ? localCommit : undefined}
		line={localCommit?.status == 'remote' || remoteLine}
		root={localLine}
		{first}
		{base}
	/>
	{#if hasLocalColumn}
		<LocalLine
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
		min-height: var(--size-16);
		&.base {
			height: var(--size-40);
		}
	}
</style>
