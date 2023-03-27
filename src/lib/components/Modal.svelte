<script lang="ts">
	import { scale } from 'svelte/transition';
	let dialog: HTMLDialogElement;
	import { onMount } from 'svelte';

	onMount(() => {
		dialog.showModal();
	});
</script>

<!-- svelte-ignore a11y-click-events-have-key-events -->
<dialog
	class="w-[640px] 
    overflow-hidden rounded-lg
    border-[0.5px] border-[#3F3F3F] bg-zinc-900/70
    p-0
	shadow-lg
	backdrop-blur-xl
    "
	in:scale={{ duration: 150 }}
	bind:this={dialog}
	on:click|self={() => dialog.close()}
	on:close
>
	<slot />
</dialog>
