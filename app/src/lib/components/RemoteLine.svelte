<script lang="ts">
	import Avatar from './Avatar.svelte';
	import { getAvatarTooltip } from '$lib/utils/avatar';
	import type { Commit, RemoteCommit } from '$lib/vbranches/types';

	export let commit: Commit | undefined;
	export let remoteCommit: RemoteCommit | undefined;
	export let base: boolean;
	export let first: boolean;
	export let short: boolean;
	export let line: boolean;
	export let root: boolean;
	export let upstreamLine: boolean;

	$: tooltipText = getAvatarTooltip(commit || remoteCommit);
</script>

<div class="remote-column" class:has-root={root}>
	{#if base}
		<div class="remote-line dashed" class:short={!line} />
		{#if root}
			<div class="root base" />
		{/if}
		<div class="commit-icon">
			<svg
				xmlns="http://www.w3.org/2000/svg"
				width="16"
				height="16"
				viewBox="0 0 16 16"
				fill="none"
			>
				<path
					fill-rule="evenodd"
					clip-rule="evenodd"
					d="M4.32501 7.25C4.67247 5.53832 6.18578 4.25 8 4.25C9.81422 4.25 11.3275 5.53832 11.675 7.25H14V8.75H11.675C11.3275 10.4617 9.81422 11.75 8 11.75C6.18578 11.75 4.67247 10.4617 4.32501 8.75H2V7.25H4.32501ZM8 5.75C6.75736 5.75 5.75 6.75736 5.75 8C5.75 9.24264 6.75736 10.25 8 10.25C9.24264 10.25 10.25 9.24264 10.25 8C10.25 6.75736 9.24264 5.75 8 5.75Z"
					fill="white"
				/>
			</svg>
		</div>
	{:else}
		{#if line}
			{#if upstreamLine}
				<div class="remote-line tip" class:upstream={upstreamLine}></div>
			{/if}
			<div class="remote-line" class:short class:first />
		{:else if upstreamLine}
			<div class="remote-line upstream" class:short class:first />
		{/if}
		{#if root}
			<div class="root" />
		{/if}
		{#if commit}
			{@const author = commit.author}
			<div class="avatar" class:first class:short>
				<Avatar {author} status={commit.status} help={tooltipText} />
			</div>
		{/if}
		{#if remoteCommit}
			{@const author = remoteCommit.author}
			<div class="avatar" class:first class:short>
				<Avatar {author} status={remoteCommit.status} help={tooltipText} />
			</div>
		{/if}
	{/if}
</div>

<style lang="postcss">
	.remote-column {
		position: relative;
		width: var(--size-24);
	}

	.remote-line {
		position: absolute;
		width: var(--size-2);
		background-color: var(--clr-commit-remote);
		left: calc(var(--size-10) + var(--size-1));
		bottom: 0;
		top: 0;
		&.first {
			top: calc(var(--size-40) + var(--size-2));
		}
		&.short {
			top: 1rem;
			&.first {
				top: 3rem;
			}
		}
		&.tip {
			bottom: calc(100% - 2.625rem);
		}
		&.dashed {
			background: repeating-linear-gradient(
				0,
				transparent,
				transparent 0.1875rem,
				var(--clr-commit-remote) 0.1875rem,
				var(--clr-commit-remote) 0.4375rem
			);
		}
		&.upstream {
			background-color: var(--clr-commit-upstream);
			top: 0;
			&.short {
				top: 1rem;
				&.first {
					top: calc(var(--size-40) + var(--size-2));
				}
			}
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

	.root {
		position: absolute;
		width: var(--size-10);
		top: 1.875rem;
		border-radius: var(--radius-l) 0 0 0;
		height: var(--size-10);
		left: calc(var(--size-10) + var(--size-1));
		border-color: var(--clr-commit-local);
		border-width: var(--size-2) 0 0 var(--size-2);
		&.base {
			top: 0;
		}
	}

	.commit-icon {
		display: inline-block;
		position: absolute;
		border-radius: 6px;
		left: var(--size-4);
		background: var(--clr-commit-remote);
		height: var(--size-16);
		width: var(--size-16);
		top: var(--size-10);
		& svg {
			height: var(--size-16);
			width: var(--size-16);
		}
	}
</style>
