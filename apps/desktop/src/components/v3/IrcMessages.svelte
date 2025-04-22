<script lang="ts">
	import ConfigurableScrollableContainer from '$components/ConfigurableScrollableContainer.svelte';
	import { SETTINGS, type Settings } from '$lib/settings/userSettings';
	import { getContextStoreBySymbol } from '@gitbutler/shared/context';
	import HunkDiff from '@gitbutler/ui/HunkDiff.svelte';
	import FileListItemV3 from '@gitbutler/ui/file/FileListItemV3.svelte';
	import type { TreeChange } from '$lib/hunks/change';
	import type { DiffHunk } from '$lib/hunks/hunk';
	import type { IrcLog } from '$lib/irc/types';

	type Props = {
		logs: IrcLog[];
	};

	const { logs }: Props = $props();

	const userSettings = getContextStoreBySymbol<Settings>(SETTINGS);
	let scroller: ConfigurableScrollableContainer;
</script>

{#snippet rowTemplate(log: IrcLog)}
	{@const timestamp = new Date(log.timestamp).toLocaleTimeString()}
	<div class="message" class:error={log.type === 'outgoing' && log.error}>
		[{timestamp}]
		{#if log.type === 'incoming' || log.type === 'outgoing'}
			{@const blah = log.data ? JSON.parse(atob(log.data as any)) : undefined}
			{@const { change, diff }: {change: TreeChange, diff: DiffHunk} = blah || {}}
			{log.from}: {log.message}
			{#if change && diff.diff}
				<FileListItemV3 filePath={change.path} listMode="list" />
				<HunkDiff
					draggingDisabled={true}
					hideCheckboxes={true}
					filePath={change.path}
					hunkStr={diff.diff}
					diffLigatures={$userSettings.diffLigatures}
					tabSize={$userSettings.tabSize}
					wrapText={$userSettings.wrapText}
					diffFont={$userSettings.diffFont}
					diffContrast={$userSettings.diffContrast}
					inlineUnifiedDiffs={$userSettings.inlineUnifiedDiffs}
				/>
			{/if}
		{:else if log.type === 'notice'}
			{log.message}
		{:else if log.type === 'server'}
			{log.message}
		{:else if log.type === 'command'}
			{log.raw}
		{/if}
	</div>
	{#if log.type === 'outgoing' && log.error}
		{log.error}
	{/if}
{/snippet}

<div class="messages">
	<ConfigurableScrollableContainer bind:this={scroller} autoScroll>
		{#each logs || [] as log}
			{@render rowTemplate(log)}
		{/each}
	</ConfigurableScrollableContainer>
</div>

<style lang="postcss">
	.messages {
		flex-grow: 1;
		overflow: hidden;
	}
	.message {
		font-family: var(--fontfamily-mono);
		white-space: pre-wrap;
		user-select: text;
		padding: 2px 6px;
	}
	.error {
		background-color: var(--clr-scale-err-90);
	}
</style>
