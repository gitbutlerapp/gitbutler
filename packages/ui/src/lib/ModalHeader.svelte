<script lang="ts">
	import Button from '$lib/Button.svelte';
	import Icon from '$lib/Icon.svelte';
	import { type ModalType } from '$lib/Modal.svelte';
	import { stickyHeader } from '@gitbutler/ui/utils/stickyHeader';
	import { type Snippet } from 'svelte';

	interface Props {
		type?: ModalType;
		closeButton?: boolean;
		sticky?: boolean;
		children: Snippet;
		oncloseclick?: () => void;
	}

	const { type = 'info', closeButton, sticky, children, oncloseclick }: Props = $props();
</script>

<div
	class="modal__header"
	use:stickyHeader={{
		disabled: !sticky
	}}
>
	{#if type === 'warning'}
		<Icon name="warning" color="warning" />
	{/if}

	{#if type === 'error'}
		<Icon name="error" color="error" />
	{/if}

	{#if type === 'success'}
		<Icon name="success" color="success" />
	{/if}

	<h2 class="text-14 text-semibold">
		{@render children()}
	</h2>

	{#if closeButton}
		<div class="close-btn">
			<Button type="button" kind="ghost" icon="cross" onclick={oncloseclick}></Button>
		</div>
	{/if}
</div>

<style lang="postcss">
	.modal__header {
		display: flex;
		position: relative;
		align-items: center;
		padding: 16px;
		gap: 8px;
		background-color: var(--clr-bg-1);
	}

	.close-btn {
		position: absolute;
		top: 10px;
		right: 10px;
	}
</style>
