<script lang="ts">
	import type { Author } from '$lib/vbranches/types';

	export let author: Author;
</script>

<img
	class="-ml-1 inline-block h-4 w-4 rounded-full first:ml-0"
	title="Gravatar for {author.email || author.name}"
	alt="Gravatar for {author.email || author.name}"
	srcset="{author.gravatarUrl} 2x"
	width="100"
	height="100"
	on:error
/>
