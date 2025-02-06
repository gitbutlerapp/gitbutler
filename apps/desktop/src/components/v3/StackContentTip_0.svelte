<script lang="ts">
	import WhatIsAStack from '$lib/assets/illustrations/tip-what-is-a-stack.svg?raw';
	import { fade } from 'svelte/transition';
</script>

<div class="placeholder__svg" in:fade={{ duration: 250 }}>
	{@html WhatIsAStack}
</div>
<div class="placeholder__text">
	<div class="text-18 text-semibold text-clr1">What is a stack?</div>
	<div class="text-13 text-clr2">
		<p>
			Stack is a workflow where branches are built sequentially, breaking large features into
			smaller parts. Each branch depends on the previous one. You can also choose a single-branch
			flow.
		</p>
	</div>
</div>

<style>
	.placeholder__svg {
		width: 300px;
	}

	.placeholder__text {
		text-align: left;
		line-height: 1 !important;
		margin: 0 auto;
		display: flex;
		flex-direction: column;
		gap: 8px;

		& p {
			line-height: 1.75;
		}
	}
</style>
