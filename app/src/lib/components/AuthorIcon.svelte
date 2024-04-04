<script lang="ts">
	import type { Author } from '$lib/vbranches/types';

	export let author: Author;

	let imgCaption = `Gravatar for ${author.email || author.name}`;
</script>

<img
	class="gravatar-image"
	title={imgCaption}
	alt={imgCaption}
	srcset="{author.gravatarUrl} 2x"
	on:error
/>

<style lang="postcss">
	.gravatar-image {
		width: var(--size-16);
		height: var(--size-16);
		border-radius: 50%;
		margin-left: calc(var(--size-4) * -1);

		&:first-child {
			margin-left: 0;
		}
	}
</style>
