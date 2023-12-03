<script lang="ts">
	import type { Author } from '$lib/vbranches/types';
	import AuthorIcon from './AuthorIcon.svelte';

	export let authors: Author[];
</script>

<div>
	{#each authors as author}
		{#if author.gravatarUrl}
			<AuthorIcon {author} />
		{/if}
	{/each}
</div>
