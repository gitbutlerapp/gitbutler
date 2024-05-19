<script lang="ts">
	import { tooltip } from '$lib/utils/tooltip';
	import type { Commit, RemoteCommit } from '$lib/vbranches/types';

	export let line: boolean;
	export let first: boolean;
	export let remoteCommit: RemoteCommit | undefined;
	export let localCommit: Commit | undefined;
	export let dashed: boolean;
	export let upstreamLine: boolean;
</script>

<div class="shadow-column">
	{#if line}
		{#if upstreamLine}
			<div class="shadow-line tip" class:upstream={upstreamLine}></div>
		{/if}
		<div class="shadow-line" class:dashed class:short={first} />
	{:else if upstreamLine}
		<div class="shadow-line upstream" class:short={first} />
	{/if}
	{#if localCommit}
		<div class="shadow-marker" class:first use:tooltip={localCommit.descriptionTitle}></div>
	{/if}
	{#if remoteCommit}
		{@const author = remoteCommit.author}
		<img
			class="avatar avatar"
			class:first
			title={author.name}
			alt="Gravatar for {author.email}"
			srcset="{author.gravatarUrl} 2x"
			width="100"
			height="100"
			on:error
		/>
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
		height: 100%;
		width: var(--size-2);
		background-color: var(--clr-commit-shadow);
		left: 75%;
		bottom: 0;
		top: 0;
		&.short {
			top: 3rem;
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
		width: var(--size-16);
		height: var(--size-16);
		border-radius: var(--radius-m);
		top: var(--size-10);
		left: var(--size-4);
		border: var(--size-2) solid var(--clr-commit-upstream);
		&.first {
			top: var(--size-40);
		}
	}
</style>
