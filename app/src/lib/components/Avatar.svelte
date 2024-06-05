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
		class:upstream={status === 'upstream'}
		class:integrated={status === 'integrated'}
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
		class:local={status === 'local'}
		class:remote={status === 'remote'}
		class:upstream={status === 'upstream'}
		class:integrated={status === 'integrated'}
		class:remote-lane={remoteLane}
		class:shadow-lane={shadowLane}
		use:tooltip={help}
		on:error
	/>
{/if}

<style lang="postcss">
	.avatar {
		position: absolute;
		width: 16px;
		min-width: 16px;
		height: 16px;
		border-radius: var(--radius-l);
		top: var(--avatar-top);
		left: -3px;

		&.remote-lane {
			left: 4px;
		}
		&.remote {
			border: 2px solid var(--clr-commit-remote);
			left: 4px;
		}
		&.local {
			border: 2px solid var(--clr-commit-local);
		}
		&.upstream {
			border: 2px solid var(--clr-commit-upstream);
		}
		&.integrated {
			border: 2px solid var(--clr-commit-shadow);
		}
		&.first {
			top: var(--avatar-first-top);
		}
		&.shadow-lane {
			left: 5px;
		}
	}

	.shadow-marker {
		position: absolute;
		width: 10px;
		height: 10px;
		border-radius: 100%;
		top: calc(var(--avatar-top) + 4px);
		left: 7px;
		background-color: var(--clr-commit-remote);
		&.integrated {
			background-color: var(--clr-commit-shadow);
		}
		&.first {
			top: calc(var(--avatar-first-top) + 3px);
		}
		&.shadow-lane {
			left: 8px;
			background-color: var(--clr-commit-shadow);
		}
		&.upstream {
			background-color: var(--clr-commit-upstream);
		}
	}
</style>
