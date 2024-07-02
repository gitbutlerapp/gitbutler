<script lang="ts" context="module">
	import type { ComponentColor } from '$lib/vbranches/types';
	export type MessageStyle = Exclude<ComponentColor, 'ghost' | 'purple'>;
</script>

<script lang="ts">
	import Button from '$lib/shared/Button.svelte';
	import Icon, { type IconColor } from '$lib/shared/Icon.svelte';
	import { createEventDispatcher } from 'svelte';
	import type iconsJson from '../icons/icons.json';

	export let icon: keyof typeof iconsJson | undefined = undefined;
	export let style: MessageStyle = 'neutral';
	export let outlined: boolean = true;
	export let filled: boolean = false;
	export let primary: string | undefined = undefined;
	export let secondary: string | undefined = undefined;
	export let shadow = false;
	export let error: string | undefined = undefined;

	const dispatch = createEventDispatcher<{ primary: void; secondary: void }>();

	const iconMap: { [Key in MessageStyle]: keyof typeof iconsJson } = {
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

	const primaryButtonMap: { [Key in MessageStyle]: ComponentColor } = {
		neutral: 'pop',
		pop: 'pop',
		warning: 'warning',
		error: 'error',
		success: 'pop'
	};
</script>

<div
	class="info-message {style}"
	class:has-border={outlined}
	class:has-background={filled}
	class:shadow
>
	<Icon name={icon ? icon : iconMap[style]} color={iconColorMap[style]} />
	<div class="info-message__inner">
		<div class="info-message__content">
			{#if $$slots.title}
				<div class="info-message__title text-base-body-13 text-semibold">
					<slot name="title" />
				</div>
			{/if}

			{#if $$slots.content}
				<div class="info-message__text text-base-body-12">
					<slot name="content" />
				</div>
			{/if}
		</div>

		{#if error}
			<code class="info-message__error-block">
				{error}
			</code>
		{/if}

		{#if primary || secondary}
			<div class="info-message__actions">
				{#if secondary}
					<Button style="ghost" outline on:click={() => dispatch('secondary')}>
						{secondary}
					</Button>
				{/if}
				{#if primary}
					<Button style={primaryButtonMap[style]} kind="solid" on:click={() => dispatch('primary')}>
						{primary}
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
		gap: 8px;
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
