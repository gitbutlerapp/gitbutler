<script lang="ts">
	import AuthorIcon from './AuthorIcon.svelte';
	import type { Author } from '$lib/vbranches/types';

	export let authors: Author[];

	let maximumAuthors = 4;
</script>

<div class="gravatar-icons">
	{#each authors as author, index}
		{#if author.gravatarUrl && index < maximumAuthors}
			<AuthorIcon {author} />
		{:else if index === maximumAuthors}
			<div class="authors-left-counter" title={authors.length - maximumAuthors + ' more authors'}>
				<span class="text-base-10 text-semibold">
					+{authors.length - maximumAuthors}
				</span>
			</div>
		{/if}
	{/each}
</div>

<style lang="postcss">
	.gravatar-icons {
		display: flex;
		align-items: center;
	}

	.authors-left-counter {
		display: flex;
		align-items: center;
		justify-content: center;
		height: 16px;
		padding: 0 4px;
		background-color: var(--clr-bg-1);
		border-radius: 16px;
		margin-left: -4px;
		border: 1px solid var(--clr-border-2);

		& span {
			color: var(--clr-scale-ntrl-0);
			line-height: 1;
		}
	}
</style>
