<script lang="ts" module>
	export type MessageStyle = "info" | "warning" | "danger" | "success";
</script>

<script lang="ts">
	import Button from "$components/Button.svelte";
	import Icon from "$components/Icon.svelte";
	import { type IconName } from "$lib/icons/names";
	import { copyToClipboard } from "$lib/utils/clipboard";
	import { type ComponentColorType } from "$lib/utils/colorTypes";
	import type { Snippet } from "svelte";

	interface Props {
		icon?: IconName;
		style?: MessageStyle;
		outlined?: boolean;
		filled?: boolean;
		class?: string;
		primaryLabel?: string;
		primaryIcon?: IconName;
		primaryTestId?: string;
		primaryAction?: () => void;
		secondaryLabel?: string;
		secondaryIcon?: IconName;
		secondaryTestId?: string;
		secondaryAction?: () => void;
		tertiaryLabel?: string;
		tertiaryTestId?: string;
		tertiaryIcon?: IconName;
		tertiaryAction?: () => void;
		shadow?: boolean;
		error?: string | undefined;
		title?: Snippet;
		content?: Snippet;
		testId?: string;
	}

	const {
		icon: iconName,
		style = "info",
		outlined = true,
		filled = false,
		class: className = "",
		primaryLabel = "",
		primaryIcon,
		primaryTestId,
		primaryAction,
		secondaryLabel = "",
		secondaryIcon,
		secondaryTestId,
		secondaryAction,
		tertiaryLabel = "",
		tertiaryIcon,
		tertiaryTestId,
		tertiaryAction,
		shadow = false,
		error,
		title,
		content,
		testId,
	}: Props = $props();

	const iconMap: { [Key in MessageStyle]: IconName } = {
		info: "info",
		warning: "warning",
		danger: "danger",
		success: "tick-circle",
	};

	const iconColorMap: { [Key in MessageStyle]: string } = {
		info: "var(--fill-pop-bg)",
		warning: "var(--fill-warn-bg)",
		danger: "var(--fill-danger-bg)",
		success: "var(--fill-safe-bg)",
	};

	const primaryButtonMap: { [Key in MessageStyle]: ComponentColorType } = {
		info: "pop",
		warning: "warning",
		danger: "danger",
		success: "pop",
	};

	const resolvedIconName = $derived(iconName ?? (iconMap[style] as IconName));
</script>

<div
	data-testid={testId}
	class="info-message {style} {className}"
	class:has-border={outlined}
	class:has-background={filled}
	class:shadow
>
	<div class="info-message__icon">
		<Icon name={resolvedIconName} color={iconColorMap[style]} />
	</div>
	<div class="info-message__inner">
		<div class="info-message__content">
			{#if title}
				<div class="info-message__title text-13 text-body text-bold">
					{@render title()}
				</div>
			{/if}

			{#if content}
				<div class="info-message__text text-12 text-body">
					{@render content()}
				</div>
			{/if}
		</div>

		{#if error}
			<code class="info-message__error-block scrollbar">
				{error}
			</code>
		{/if}

		{#if primaryLabel || secondaryLabel}
			<div class="info-message__actions">
				{#if error}
					<Button kind="ghost" onclick={() => copyToClipboard(error)} icon="copy">
						Copy error message
					</Button>
				{/if}
				{#if tertiaryLabel}
					<Button
						kind="outline"
						testId={tertiaryTestId}
						onclick={() => tertiaryAction?.()}
						icon={tertiaryIcon}
					>
						{tertiaryLabel}
					</Button>
				{/if}
				{#if secondaryLabel}
					<Button
						kind="outline"
						testId={secondaryTestId}
						onclick={() => secondaryAction?.()}
						icon={secondaryIcon}
					>
						{secondaryLabel}
					</Button>
				{/if}
				{#if primaryLabel}
					<Button
						style={primaryButtonMap[style]}
						onclick={() => primaryAction?.()}
						icon={primaryIcon}
						testId={primaryTestId}
					>
						{primaryLabel}
					</Button>
				{/if}
			</div>
		{/if}
	</div>
</div>

<style lang="postcss">
	.info-message {
		display: flex;
		width: 100%;
		padding: 14px;
		gap: 12px;
		border-radius: var(--radius-m);
		background-color: var(--bg-1);
		color: var(--text-1);
		transition: background-color var(--transition-slow);
	}
	.info-message__inner {
		display: flex;
		flex-grow: 1;
		flex-direction: column;
		overflow: hidden;
		gap: 12px;
	}
	.info-message__content {
		display: flex;
		flex-direction: column;
		gap: 6px;
		user-select: text;
	}
	.info-message__icon {
		display: flex;
		flex-shrink: 0;
		padding: 2px 0;
	}
	.info-message__actions {
		display: flex;
		justify-content: flex-end;
		gap: 6px;
	}
	.info-message__text {
		opacity: 60%;
		&:empty {
			display: none;
		}
	}

	/* MODIFIERS */
	.info {
		border: 0 solid var(--border-2);
	}
	.danger {
		border: 0 solid var(--fill-danger-bg);
	}
	.warning {
		border: 0 solid var(--fill-warn-bg);
	}
	.success {
		border: 0 solid var(--fill-safe-bg);
	}
	.shadow {
		box-shadow: 0px 7px 14px 0px rgba(0, 0, 0, 0.1);
	}

	/* OUTLINED */
	.has-border {
		border-width: 1px;
	}

	.has-background {
		&.info {
			background-color: var(--bg-2);
		}
		&.danger {
			background-color: var(--bg-danger);
		}
		&.warning {
			background-color: var(--bg-warn);
		}
		&.success {
			background-color: var(--bg-safe);
		}
	}

	/* ERROR BLOCK */
	.info-message__error-block {
		max-height: 350px;
		padding: 10px;
		overflow: auto;
		border-radius: var(--radius-s);
		background-color: var(--bg-danger);
		color: var(--text-danger);
		font-size: 12px;
		white-space: pre-wrap;
		overflow-wrap: break-word;
		user-select: text;

		/* selection */
		&::selection {
			background-color: color-mix(in srgb, var(--fill-danger-bg) 20%, transparent);
		}
		/* empty */
		&:empty {
			display: none;
		}
	}

	/* rendered markdown requires global */
	:global(.info-message__text a) {
		text-decoration: underline;
		word-break: break-all; /* allow long links to wrap */
		cursor: pointer;
	}
	:global(.info-message__text p:not(:last-child)) {
		margin-bottom: 10px;
	}
	:global(.info-message__text ul) {
		padding: 0 0 0 16px;
		list-style-type: circle;
	}
</style>
