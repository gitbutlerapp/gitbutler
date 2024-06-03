<script lang="ts">
	import { tooltip } from '$lib/utils/tooltip';
	import type { Author, CommitStatus } from '$lib/vbranches/types';

	export let author: Author | undefined;
	export let status: CommitStatus | undefined;
	export let help: string | undefined = undefined;

	export let shadow = false;
	export let remoteLane = false;
	export let shadowLane = false;
	export let sectionFirst = false;
</script>

{#if shadow}
	<div
		class="shadow-marker"
		class:first={sectionFirst}
		class:upstream={status == 'upstream'}
		class:integrated={status == 'integrated'}
		class:shadow-lane={shadowLane}
		use:tooltip={help}
	/>
{:else}
	<img
		class="avatar"
		alt="Gravatar for {author?.email}"
		srcset="{author?.gravatarUrl} 2x"
		width="100"
		height="100"
		class:first={sectionFirst}
		class:local={status == 'local'}
		class:remote={status == 'remote'}
		class:upstream={status == 'upstream'}
		class:integrated={status == 'integrated'}
		class:remote-lane={remoteLane}
		class:shadow-lane={shadowLane}
		use:tooltip={help}
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
			background-color: var(--clr-commit-shadow);
		}
		&.upstream {
			background-color: var(--clr-commit-upstream);
		}
	}
</style>
