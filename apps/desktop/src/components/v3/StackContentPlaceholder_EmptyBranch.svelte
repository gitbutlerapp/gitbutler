<script lang="ts">
	import EmptyStack from '$lib/assets/illustrations/empty-stack-placeholder.svg?raw';
	import { fade } from 'svelte/transition';
</script>

<div class="placeholder__svg" in:fade={{ duration: 250 }}>
	{@html EmptyStack}
</div>
<div class="placeholder__text">
	<div class="text-18 text-semibold text-clr1">This is a new stack</div>
	<div class="text-13 text-clr2">
		<p>
			Stack is a workflow for building branches sequentially to break features into smaller parts.
		</p>
		<p>You can also choose a regular single-branch flow.</p>
	</div>
</div>

<style>
	.placeholder__svg {
		height: 200px;
		width: 300px;
	}

	.placeholder__text {
		text-align: left;
		line-height: 1 !important;
		display: inline;

		white-space: pre-wrap;
		text-wrap: wrap;
	}

	p {
		display: inline-block;
		line-height: 1.5;
	}
</style>
