<script lang="ts">
	import ConfigurableScrollableContainer from '$components/ConfigurableScrollableContainer.svelte';
	import { SETTINGS } from '$lib/settings/userSettings';
	import { inject } from '@gitbutler/shared/context';
	import { HunkDiff, FileListItem } from '@gitbutler/ui';
	import type { TreeChange } from '$lib/hunks/change';
	import type { DiffHunk } from '$lib/hunks/hunk';
	import type { IrcLog } from '$lib/irc/types';

	type Props = {
		logs: IrcLog[];
	};

	const { logs }: Props = $props();

	const userSettings = inject(SETTINGS);
	let scroller: ConfigurableScrollableContainer;
</script>

{#snippet logTemplate(log: IrcLog)}
	{@const timestamp = new Date(log.timestamp).toLocaleTimeString()}
	<div class="message text-12" class:error={log.type === 'outgoing' && log.error}>
		[{timestamp}]
		{#if log.type === 'incoming' || log.type === 'outgoing'}
			{@const blah = log.data ? JSON.parse(atob(log.data as any)) : undefined}
			{@const { change, diff }: {change: TreeChange, diff: DiffHunk} = blah || {}}
			{log.from}: {log.message}
			{#if change && diff.diff}
				<div class="extra">
					<FileListItem filePath={change.path} listMode="list" hideBorder />
					<div class="diff">
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
					</div>
				</div>
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
			{@render logTemplate(log)}
		{/each}
	</ConfigurableScrollableContainer>
</div>

<style lang="postcss">
	.messages {
		flex-grow: 1;
		overflow: hidden;
	}
	.message {
		padding: 0 12px;
		font-family: var(--fontfamily-mono);
		white-space: pre-wrap;
		user-select: text;
	}
	.error {
		background-color: var(--clr-scale-err-90);
	}
	.extra {
		padding: 6px 0;
	}
	.diff {
		padding: 6px 0px 12px 0px;
		white-space: initial;
	}
</style>
