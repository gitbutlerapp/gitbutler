<script lang="ts">
	import type { Author } from '$lib/vbranches/types';

	export let authors: Author[];
</script>

<div>
	{#each authors as author}
		<img
			class="-ml-1 inline-block h-4 w-4 rounded-full first:ml-0"
			title="Gravatar for {author.email}"
			alt="Gravatar for {author.email}"
			srcset="{author.gravatarUrl} 2x"
			width="100"
			height="100"
			on:error
		/>
	{/each}
</div>
