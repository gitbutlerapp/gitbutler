<script>
	import { IconArrowBigLeftFilled, IconArrowBigRightFilled } from '@tabler/icons-svelte';
	let history = window.history;
</script>

<div class="flex items-center justify-center space-x-3 text-zinc-400">
	<button class="hover:text-zinc-200" title="Go back" on:click={() => history.back()}>
		<IconArrowBigLeftFilled class="w-4 h-4  " />
	</button>
	<button class="hover:text-zinc-200" title="Go forward" on:click={() => history.forward()}>
		<IconArrowBigRightFilled class="w-4 h-4  " />
	</button>
</div>
