<script lang="ts">
	import { Button, Modal, Link } from '$lib/components';
	import { onMount } from 'svelte';
	import { IconGitBranch } from './icons';

	export const show = () => modal.show();

	let modal: Modal;

	onMount(() => {
		modal.show();
	});
</script>

<Modal bind:this={modal} let:close>
	<form
		class="font-modal-stroke/50 flex w-[680px] flex-col gap-6 rounded-lg border-[0.5px] border-modal-stroke bg-modal-background p-4"
	>
		<header class="flex w-full items-center justify-between">
			<h2 class="flex items-center gap-4">
				<IconGitBranch class="h-5 w-5" />
				<span class="line-height-5 text-white">ian/gb-1234-this-is-a-big-update</span>
			</h2>

			<Button role="purple" height="small">Autowrite</Button>
		</header>

		<div class="flex flex-col gap-2">
			<input
				name="commit-message"
				class="border-none bg-transparent p-0 text-xl text-zinc-100"
				type="text"
				placeholder="Commit message (required)"
				required
			/>

			<textarea
				name="commit-description"
				class="border-none bg-transparent p-0 text-lg text-zinc-400"
				placeholder="Commit description (optional)"
				rows="6"
			/>
		</div>

		<footer class="flex items-center justify-between">
			<div class="text-zinc-400">
				<Link href="/">8 files changed</Link>
			</div>
			<div class="flex gap-2">
				<Button filled={false} outlined={true} on:click={close}>Cancel</Button>
				<Button role="primary" on:click={() => alert('commit!')}>Commit</Button>
			</div>
		</footer>
	</form>
</Modal>
