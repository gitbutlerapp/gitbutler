<script lang="ts">
	import { tooltip } from '$lib/utils/tooltip';
	import type { Author, CommitStatus } from '$lib/vbranches/types';

	export let author: Author;
	export let status: CommitStatus;
</script>

<img
	class="avatar"
	alt="Gravatar for {author.email}"
	srcset="{author.gravatarUrl} 2x"
	width="100"
	height="100"
	class:remote={status == 'remote'}
	class:local={status == 'local'}
	class:upstream={status == 'upstream'}
	use:tooltip={author.name}
	on:error
/>

<style lang="postcss">
	.avatar {
		width: var(--size-16);
		height: var(--size-16);
		border-radius: var(--radius-m);
	}

	.local {
		border: var(--size-2) solid var(--clr-commit-local);
	}
	.remote {
		border: var(--size-2) solid var(--clr-commit-remote);
	}
	.upstream {
		border: var(--size-2) solid var(--clr-commit-upstream);
	}
</style>
