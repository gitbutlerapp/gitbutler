<script lang="ts">
	import Button from '$lib/shared/Button.svelte';
	import Modal from '$lib/shared/Modal.svelte';

	export let projectTitle: string = '#';
	export let isDeleting = false;
	export let onDeleteClicked: () => Promise<void>;

	export function show() {
		modal.show();
	}
	export function close() {
		modal.close();
	}

	let modal: Modal;
</script>

<Button
	style="error"
	kind="solid"
	icon="bin-small"
	reversedDirection
	on:click={() => {
		modal.show();
	}}
>
	Remove project…
</Button>

<Modal bind:this={modal} width="small">
	<div class="remove-project-description">
		<p class="text-base-body-14">
			Are you sure you want to remove
			<span class="text-bold">{projectTitle}</span> from GitButler?
		</p>

		<p class="text-base-body-12 details-text">
			When you delete your project from GitButler, your repository doesn't get deleted. It just
			removes the project from the list, keeping your repository safe and easy to access.
		</p>
	</div>

	{#snippet controls(close)}
		<Button
			style="error"
			kind="solid"
			reversedDirection
			loading={isDeleting}
			icon="bin-small"
			on:click={() => {
				onDeleteClicked().then(close);
			}}
		>
			Remove
		</Button>
		<Button style="pop" kind="solid" on:click={close}>Cancel</Button>
	{/snippet}
</Modal>

<style lang="postcss">
	.remove-project-description {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}

	.details-text {
		opacity: 0.5;
	}
</style>
