<script lang="ts">
	import { getAvatarTooltip } from '$lib/utils/avatar';
	import { tooltip } from '$lib/utils/tooltip';
	import type { Commit, RemoteCommit } from '$lib/vbranches/types';

	export let commit: Commit | RemoteCommit | undefined;
	export let shadow: boolean = false;
	export let remoteLane: boolean = false;
	export let shadowLane: boolean = false;
	export let first = false;

	$: tooltipText = getAvatarTooltip(commit);
</script>

{#if shadow}
	<div
		class:first
		class="shadow-marker"
		class:upstream={commit?.status == 'upstream'}
		class:integrated={commit?.status == 'integrated'}
		class:shadow-lane={shadowLane}
		use:tooltip={tooltipText}
	></div>
{:else}
	<img
		class="avatar"
		alt="Gravatar for {commit?.author.email}"
		srcset="{commit?.author.gravatarUrl} 2x"
		width="100"
		height="100"
		class:first
		class:local={commit?.status == 'local'}
		class:remote={commit?.status == 'remote'}
		class:upstream={commit?.status == 'upstream'}
		class:integrated={commit?.status == 'integrated'}
		class:remote-lane={remoteLane}
		class:shadow-lane={shadowLane}
		use:tooltip={tooltipText}
		on:error
	/>
{/if}

<style lang="postcss">
	.avatar {
		position: absolute;
		width: var(--size-16);
		min-width: var(--size-16);
		height: var(--size-16);
		border-radius: var(--radius-l);
		top: var(--avatar-top);
		left: calc(-1 * (var(--size-2) + 0.063rem));
		&.remote-lane {
			left: var(--size-4);
		}

		&.remote {
			border: var(--size-2) solid var(--clr-commit-remote);
			left: var(--size-4);
		}
		&.local {
			border: var(--size-2) solid var(--clr-commit-local);
		}
		&.upstream {
			border: var(--size-2) solid var(--clr-commit-upstream);
		}
		&.integrated {
			border: var(--size-2) solid var(--clr-commit-shadow);
			left: var(--size-4);
		}
		&.first {
			top: var(--avatar-first-top);
		}
		&.shadow-lane {
			left: calc(var(--size-4) + 0.063rem);
		}
	}

	.shadow-marker {
		position: absolute;
		width: var(--size-10);
		height: var(--size-10);
		border-radius: 100%;
		top: calc(var(--avatar-top) + var(--size-4));
		left: calc(var(--size-6) + 0.063rem);
		background-color: var(--clr-commit-remote);
		&.integrated {
			background-color: var(--clr-commit-shadow);
		}
		&.first {
			top: calc(var(--avatar-first-top) + var(--size-2) + 0.063rem);
		}
		&.shadow-lane {
			left: var(--size-8);
		}
	}
</style>
