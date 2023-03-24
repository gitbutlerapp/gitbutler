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
	class="rounded-lg 
    border border-zinc-400/40
    bg-zinc-900/70 p-0 shadow-lg
    backdrop-blur-xl
    "
	in:scale={{ duration: 150 }}
	bind:this={dialog}
	on:click|self={() => dialog.close()}
	on:close
>
	<slot />
</dialog>
