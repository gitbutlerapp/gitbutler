<script lang="ts">
	import Modal from '../Modal.svelte';
	import { onMount } from 'svelte';
	let modal: Modal;
	onMount(() => {
		modal.show();
	});
</script>

<Modal on:close bind:this={modal}>
	<h1>Branch</h1>
</Modal>
