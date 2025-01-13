<script lang="ts" module>
	import type { ComponentColorType } from '@gitbutler/ui/utils/colorTypes';
	export type MessageStyle = Exclude<ComponentColorType, 'ghost' | 'purple'>;
</script>

<script lang="ts">
	import Button from '@gitbutler/ui/Button.svelte';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import type iconsJson from '@gitbutler/ui/data/icons.json';
	import type { Snippet } from 'svelte';

	type IconColor = ComponentColorType | undefined;
	type IconName = keyof typeof iconsJson;

	interface Props {
		icon?: IconName | undefined;
		style?: MessageStyle;
		outlined?: boolean;
		filled?: boolean;
		primaryLabel?: string | undefined;
		primaryIcon?: IconName | undefined;
		primaryAction?: () => void;
		secondaryLabel?: string | undefined;
		secondaryIcon?: IconName | undefined;
		secondaryAction?: () => void;
		shadow?: boolean;
		error?: string | undefined;
		title?: Snippet;
		content?: Snippet;
	}

	const {
		icon: iconName = undefined,
		style = 'neutral',
		outlined = true,
		filled = false,
		primaryLabel = '',
		primaryIcon = undefined,
		primaryAction,
		secondaryLabel = '',
		secondaryIcon = undefined,
		secondaryAction,
		shadow = false,
		error = undefined,
		title,
		content
	}: Props = $props();

	const iconMap: { [Key in MessageStyle]: IconName } = {
		neutral: 'info',
		pop: 'info',
		warning: 'warning',
		error: 'error',
		success: 'success'
	};

	const iconColorMap: { [Key in MessageStyle]: IconColor } = {
		neutral: 'pop',
		pop: 'pop',
		warning: 'warning',
		error: 'error',
		success: 'success'
	};

	const primaryButtonMap: { [Key in MessageStyle]: ComponentColorType } = {
		neutral: 'pop',
		pop: 'pop',
		warning: 'warning',
		error: 'error',
		success: 'pop'
	};

	const resolvedIconName = iconName ?? (iconMap[style] as IconName);
</script>

<div
	class="info-message {style}"
	class:has-border={outlined}
	class:has-background={filled}
	class:shadow
>
	<Icon name={resolvedIconName} color={iconColorMap[style]} />
	<div class="info-message__inner">
		<div class="info-message__content">
			{#if title}
				<div class="info-message__title text-13 text-body text-semibold">
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
			<code class="info-message__error-block">
				{error}
			</code>
		{/if}

		{#if primaryLabel || secondaryLabel}
			<div class="info-message__actions">
				{#if secondaryLabel}
					<Button kind="outline" onclick={() => secondaryAction?.()} icon={secondaryIcon}>
						{secondaryLabel}
					</Button>
				{/if}
				{#if primaryLabel}
					<Button
						style={primaryButtonMap[style]}
						kind="solid"
						onclick={() => primaryAction?.()}
						icon={primaryIcon}
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
		color: var(--clr-scale-ntrl-0);
		display: flex;
		padding: 14px;
		border-radius: var(--radius-m);
		gap: 12px;
		background-color: var(--clr-bg-1);
		transition:
			background-color var(--transition-slow),
			border-color var(--transition-slow);
	}
	.info-message__inner {
		display: flex;
		flex-grow: 1;
		flex-direction: column;
		gap: 12px;
		overflow-x: hidden;
	}
	.info-message__content {
		display: flex;
		flex-direction: column;
		gap: 6px;
		user-select: text;
	}
	.info-message__actions {
		display: flex;
		gap: 6px;
		justify-content: flex-end;
	}
	.info-message__text {
		&:empty {
			display: none;
		}
	}

	.info-message__text :global(pre) {
		white-space: pre-wrap;
	}

	/* MODIFIERS */
	.neutral {
		border: 0 solid var(--clr-border-2);
	}
	.error {
		border: 0 solid var(--clr-scale-err-60);
	}
	.pop {
		border: 0 solid var(--clr-scale-pop-50);
	}
	.warning {
		border: 0 solid var(--clr-scale-warn-60);
	}
	.success {
		border: 0 solid var(--clr-scale-succ-60);
	}
	.shadow {
		box-shadow: 0px 7px 14px 0px rgba(0, 0, 0, 0.1);
	}

	/* OUTLINED */

	.has-border {
		border-width: 1px;
	}

	.has-background {
		&.neutral {
			background-color: var(--clr-bg-2);
		}

		&.error {
			background-color: var(--clr-theme-err-bg);
		}

		&.pop {
			background-color: var(--clr-theme-pop-bg);
		}

		&.warning {
			background-color: var(--clr-theme-warn-bg);
		}

		&.success {
			background-color: var(--clr-theme-succ-bg);
		}
	}

	/* ERROR BLOCK */
	.info-message__error-block {
		user-select: auto;
		padding: 4px 8px;
		overflow-x: auto;
		background-color: var(--clr-scale-err-90);
		color: var(--clr-scale-err-10);
		border-radius: var(--radius-s);
		font-size: 12px;

		/* scrollbar */
		&::-webkit-scrollbar {
			display: none;
		}

		/* selection */
		&::selection {
			background-color: var(--clr-scale-err-80);
		}

		/* empty */
		&:empty {
			display: none;
		}
	}

	/* rendered markdown requires global */
	:global(.info-message__text a) {
		cursor: pointer;
		text-decoration: underline;
		word-break: break-all; /* allow long links to wrap */
	}
	:global(.info-message__text p:not(:last-child)) {
		margin-bottom: 10px;
	}
	:global(.info-message__text ul) {
		list-style-type: circle;
		padding: 0 0 0 16px;
	}
</style>
