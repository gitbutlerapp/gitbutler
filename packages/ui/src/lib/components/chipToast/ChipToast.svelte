<script lang="ts">
	import Icon from "$components/Icon.svelte";
	import { type IconName } from "$lib/icons/names";
	import { fly, fade } from "svelte/transition";
	import type { ChipToastType, ChipToastButtonConfig } from "$components/chipToast/chipToastTypes";

	interface Props {
		type: ChipToastType;
		message: string;
		customButton?: ChipToastButtonConfig;
		showDismiss?: boolean;
		onDismiss?: () => void;
	}

	const { type, message, customButton, showDismiss, onDismiss }: Props = $props();

	function getEmojiForType(type: ChipToastType): {
		name: IconName;
		color: string;
	} {
		switch (type) {
			case "success":
				return { name: "tick-circle", color: "var(--fill-safe-bg)" };
			case "warning":
				return { name: "warning", color: "var(--fill-warn-bg)" };
			case "danger":
				return { name: "danger", color: "var(--fill-danger-bg)" };
			default:
				return { name: "info", color: "var(--fill-pop-bg)" };
		}
	}

	const icon = getEmojiForType(type);

	function handleDismiss() {
		onDismiss?.();
	}
</script>

<div
	class="text-12 chip-toast chip-toast--{type}"
	role="alert"
	aria-live="polite"
	in:fly={{ y: 20, duration: 300 }}
	out:fade={{ duration: 200 }}
>
	<div class="chip-toast__content">
		<div class="chip-toast__icon" style:--icon-toast-color={icon.color}>
			<Icon name={icon.name} />
		</div>
		<span class="chip-toast__message">{message}</span>
	</div>

	{#if customButton || showDismiss}
		<div class="text-bold chip-toast__actions">
			{#if customButton}
				<button
					type="button"
					class="chip-toast__button chip-toast__button--primary"
					onclick={customButton.action}
				>
					{customButton.label}
				</button>
			{/if}
			{#if showDismiss}
				<button
					type="button"
					class="chip-toast__button chip-toast__button--secondary"
					onclick={handleDismiss}
				>
					Dismiss
				</button>
			{/if}
		</div>
	{/if}
</div>

<style>
	.chip-toast {
		--toast-padding: 8px 12px;
		display: flex;
		width: fit-content;
		border-radius: var(--radius-m);
		background: var(--bg-toast);
		box-shadow: var(--fx-shadow-m);
		color: var(--fill-gray-fg);
		text-align: center;
	}

	/* CONTENT */
	.chip-toast__content {
		display: flex;
		align-items: center;
		justify-content: center;
		padding: var(--toast-padding);
		gap: 10px;
	}

	.chip-toast__icon {
		display: flex;
		flex-shrink: 0;
		margin-left: -2px;
		color: var(--icon-toast-color);
	}

	.chip-toast__message,
	.chip-toast__button {
		text-wrap: nowrap;
		user-select: none;
	}

	/* ACTIONS */
	.chip-toast__actions {
		display: flex;
		justify-content: center;
	}

	.chip-toast__button {
		padding: var(--toast-padding);
		border-left: 1px solid color-mix(in srgb, var(--border-2) 30%, transparent);
		transition: opacity var(--transition-fast);

		&:hover {
			opacity: 0.8;
		}
	}
</style>
