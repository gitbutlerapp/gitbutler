<script lang="ts" module>
	export type MessageStyle = 'info' | 'warning' | 'danger' | 'success';
</script>

<script lang="ts">
	import Button from '$components/Button.svelte';
	import Icon from '$components/Icon.svelte';
	import { copyToClipboard } from '$lib/utils/clipboard';

	import type iconsJson from '$lib/data/icons.json';
	import type { ComponentColorType } from '$lib/utils/colorTypes';
	import type { Snippet } from 'svelte';

	type IconColor = ComponentColorType | undefined;
	type IconName = keyof typeof iconsJson;

	interface Props {
		icon?: IconName | undefined;
		style?: MessageStyle;
		outlined?: boolean;
		filled?: boolean;
		class?: string;
		primaryLabel?: string | undefined;
		primaryIcon?: IconName | undefined;
		primaryTestId?: string | undefined;
		primaryAction?: () => void;
		secondaryLabel?: string | undefined;
		secondaryIcon?: IconName | undefined;
		secondaryTestId?: string | undefined;
		secondaryAction?: () => void;
		tertiaryLabel?: string | undefined;
		tertiaryTestId?: string | undefined;
		tertiaryIcon?: IconName | undefined;
		tertiaryAction?: () => void;
		shadow?: boolean;
		error?: string | undefined;
		title?: Snippet;
		content?: Snippet;
		testId?: string;
	}

	const {
		icon: iconName,
		style = 'info',
		outlined = true,
		filled = false,
		class: className = '',
		primaryLabel = '',
		primaryIcon,
		primaryTestId,
		primaryAction,
		secondaryLabel = '',
		secondaryIcon,
		secondaryTestId,
		secondaryAction,
		tertiaryLabel = '',
		tertiaryIcon,
		tertiaryTestId,
		tertiaryAction,
		shadow = false,
		error,
		title,
		content,
		testId
	}: Props = $props();

	const iconMap: { [Key in MessageStyle]: IconName } = {
		info: 'info',
		warning: 'warning',
		danger: 'error',
		success: 'success'
	};

	const iconColorMap: { [Key in MessageStyle]: IconColor } = {
		info: 'pop',
		warning: 'warning',
		danger: 'danger',
		success: 'safe'
	};

	const primaryButtonMap: { [Key in MessageStyle]: ComponentColorType } = {
		info: 'pop',
		warning: 'warning',
		danger: 'danger',
		success: 'pop'
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
					<Button kind="ghost" onclick={() => copyToClipboard(error)} icon="copy-small">
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
		background-color: var(--clr-bg-1);
		color: var(--clr-text-1);
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
		border: 0 solid var(--clr-border-2);
	}
	.error {
		border: 0 solid var(--clr-theme-danger-element);
	}
	.warning {
		border: 0 solid var(--clr-theme-warn-element);
	}
	.success {
		border: 0 solid var(--clr-theme-safe-element);
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
			background-color: var(--clr-bg-2);
		}
		&.error {
			background-color: var(--clr-theme-danger-bg-muted);
		}
		&.warning {
			background-color: var(--clr-theme-warn-bg-muted);
		}
		&.success {
			background-color: var(--clr-theme-safe-bg-muted);
		}
	}

	/* ERROR BLOCK */
	.info-message__error-block {
		padding: 10px 10px 0;
		overflow-x: scroll;
		border-radius: var(--radius-s);
		background-color: var(--clr-theme-danger-bg);
		color: var(--clr-theme-danger-text);
		font-size: 12px;
		white-space: pre;
		user-select: text;

		/* selection */
		&::selection {
			background-color: color-mix(in srgb, var(--clr-theme-danger-element) 20%, transparent);
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
