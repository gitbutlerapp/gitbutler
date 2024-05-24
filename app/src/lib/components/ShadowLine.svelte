<script lang="ts">
	import Avatar from './Avatar.svelte';
	import { getAvatarTooltip } from '$lib/utils/avatar';
	import { tooltip } from '$lib/utils/tooltip';
	import type { Commit, RemoteCommit } from '$lib/vbranches/types';

	export let line: boolean;
	export let first: boolean;
	export let short: boolean;
	export let remoteCommit: RemoteCommit | undefined;
	export let localCommit: Commit | undefined;
	export let dashed: boolean;
	export let upstreamLine: boolean;

	$: tooltipText = getAvatarTooltip(localCommit || remoteCommit);
</script>

<div class="shadow-column">
	{#if line}
		{#if upstreamLine}
			<div class="shadow-line tip" class:upstream={upstreamLine}></div>
		{/if}
		<div class="shadow-line" class:dashed class:short class:first />
	{:else if upstreamLine}
		<div class="shadow-line upstream" class:short class:first />
	{/if}
	{#if localCommit}
		<div class="shadow-marker" class:first class:short use:tooltip={tooltipText}></div>
	{/if}
	{#if remoteCommit}
		{@const author = remoteCommit.author}
		<div class="avatar" class:first class:short>
			<Avatar {author} status={remoteCommit.status} help={tooltipText} />
		</div>
	{/if}
</div>

<style lang="postcss">
	.shadow-column {
		position: relative;
	}

	.shadow-column {
		width: var(--size-16);
	}

	.shadow-line {
		position: absolute;
		width: var(--size-2);
		background-color: var(--clr-commit-shadow);
		left: 75%;
		bottom: 0;
		top: 0;
		&.short {
			top: 1rem;
			&.first {
				top: 3rem;
			}
		}
		&.dashed {
			background: repeating-linear-gradient(
				0,
				transparent,
				transparent 0.1875rem,
				var(--clr-commit-shadow) 0.1875rem,
				var(--clr-commit-shadow) 0.4375rem
			);
		}
		&.tip {
			bottom: calc(100% - 2.625rem);
		}
		&.upstream {
			background-color: var(--clr-commit-upstream);
		}
	}

	.shadow-marker {
		position: absolute;
		width: var(--size-10);
		height: var(--size-10);
		border-radius: 100%;
		background-color: var(--clr-commit-shadow);
		top: var(--size-14);
		left: 50%;
		&.first {
			top: 2.75rem;
		}
	}

	.avatar {
		position: absolute;
		top: var(--size-10);
		left: var(--size-4);
		&.first {
			top: calc(var(--size-40) + var(--size-2));
		}
	}
</style>
