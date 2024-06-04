<script lang="ts">
	import Avatar from './Avatar.svelte';
	import LocalLine from './LocalLine.svelte';
	import RemoteLine from './RemoteLine.svelte';
	import ShadowLine from './ShadowLine.svelte';
	import type { Author, CommitStatus } from '$lib/vbranches/types';

	export let hasLocalColumn = false;
	export let isRebased = false;

	export let sectionFirst = false;

	export let localIn: CommitStatus | undefined = undefined;
	export let localOut: CommitStatus | undefined = undefined;

	export let remoteIn: CommitStatus | undefined = undefined;
	export let remoteOut: CommitStatus | undefined = undefined;

	export let shadowIn: CommitStatus | undefined = undefined;
	export let shadowOut: CommitStatus | undefined = undefined;

	export let inDashed = false;
	export let outDashed = false;

	export let base = false;
	export let last = false;
	export let localRoot = false;
	export let integrated = false;
	export let relatedToOther = false;
	export let remoteRoot = false;

	export let help: string | undefined = undefined;
	export let shadowHelp: string | undefined = undefined;
	export let author: Author | undefined = undefined;
	export let commitStatus: CommitStatus | undefined = undefined;
</script>

<div class="lines">
	{#if isRebased}
		<ShadowLine
			inType={shadowIn}
			outType={shadowOut}
			{sectionFirst}
			outDashed={base}
			inDashed={base}
		>
			{#if author}
				<Avatar
					{author}
					{sectionFirst}
					status={shadowIn}
					help={shadowHelp || help}
					shadow={commitStatus && commitStatus !== 'upstream'}
					shadowLane
				/>
			{/if}
		</ShadowLine>
	{/if}
	<RemoteLine
		{base}
		{sectionFirst}
		root={localRoot}
		inType={remoteIn}
		outType={remoteOut}
		outDashed={remoteOut === 'integrated'}
		inDashed={remoteIn === 'integrated'}
		{integrated}
	>
		{#if !isRebased && (relatedToOther || commitStatus !== 'local')}
			<Avatar
				{author}
				{sectionFirst}
				status={commitStatus}
				help={shadowHelp || help}
				shadow={relatedToOther}
				remoteLane
			/>
		{/if}
	</RemoteLine>

	{#if hasLocalColumn}
		<LocalLine
			{inDashed}
			{outDashed}
			{sectionFirst}
			inType={localIn}
			outType={localOut}
			root={remoteRoot}
			longRoot={remoteRoot && !last}
		>
			{#if commitStatus === 'local'}
				<Avatar {help} {sectionFirst} {author} status={commitStatus} />
			{/if}
		</LocalLine>
	{/if}
</div>

<style lang="postcss">
	.lines {
		display: flex;
		align-items: stretch;
		min-height: 12px;
		padding-left: 8px;
		/* margin-top: -1px; */
	}
</style>
