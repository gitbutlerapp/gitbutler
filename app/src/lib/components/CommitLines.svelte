<script lang="ts">
	import Avatar from './Avatar.svelte';
	import LocalLine from './LocalLine.svelte';
	import RemoteLine from './RemoteLine.svelte';
	import ShadowLine from './ShadowLine.svelte';
	import { getAvatarTooltip } from '$lib/utils/avatar';
	import type { Commit, CommitStatus, RemoteCommit } from '$lib/vbranches/types';

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
	export let upstreamType: CommitStatus | undefined = undefined;

	$: root = localRoot || (integratedOrRemote && nextCommitIsLocal);
	$: nextCommitIsLocal = localCommit?.children?.[0]?.status == 'local';
	$: integratedOrRemote = localCommit?.status == 'remote' || localCommit?.status == 'integrated';
	$: lastLocalCommit = !!localCommit && !localCommit?.children?.[0];
	$: lastRemoteCommit = !!remoteCommit && !remoteCommit?.children?.[0];

	$: short = !upstreamType && (lastLocalCommit || lastRemoteCommit);
	$: relatedToOther = localCommit?.relatedTo && localCommit.relatedTo.id != localCommit.id;
	$: upstreamIsNext = !!localCommit?.relatedTo && upstreamType == 'upstream';
	$: tooltipText = getAvatarTooltip(localCommit || remoteCommit);

	$: commitStatus = localCommit?.status || remoteCommit?.status;
	$: author = localCommit?.author || remoteCommit?.author;
</script>

<div class="lines">
	{#if hasShadowColumn}
		<ShadowLine line={shadowLine} dashed={base} {upstreamLine} {upstreamType} {first} {short}>
			<Avatar
				{first}
				{author}
				help={tooltipText}
				status={commitStatus}
				shadow={!!localCommit}
				shadowLane
			/>
		</ShadowLine>
	{/if}
	<RemoteLine
		{root}
		{base}
		{first}
		{upstreamType}
		line={remoteLine}
		commit={localCommit}
		short={root || short || upstreamIsNext}
		upstreamLine={upstreamLine && !hasShadowColumn}
	>
		{#if relatedToOther || remoteCommit}
			<Avatar
				{first}
				{author}
				help={tooltipText}
				status={commitStatus}
				shadow={relatedToOther}
				remoteLane
			/>
		{/if}
	</RemoteLine>

	{#if hasLocalColumn}
		<LocalLine
			{first}
			isEmpty={base}
			dashed={localLine}
			commit={localCommit?.status == 'local' ? localCommit : undefined}
		>
			<Avatar {first} {author} help={tooltipText} status={commitStatus} />
		</LocalLine>
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
