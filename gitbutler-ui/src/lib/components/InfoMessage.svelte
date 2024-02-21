<script lang="ts" context="module">
	export type MessageStyle = 'neutral' | 'error' | 'pop' | 'warn';
</script>

<script lang="ts">
	import Button, { type ButtonColor } from './Button.svelte';
	import Icon, { type IconColor } from '$lib/components/Icon.svelte';
	import { createEventDispatcher } from 'svelte';
	import type iconsJson from '../icons/icons.json';

	export let style: MessageStyle = 'neutral';
	export let title: string | undefined = undefined;
	export let primary: string | undefined = undefined;
	export let secondary: string | undefined = undefined;
	export let shadow = false;

	const dispatch = createEventDispatcher<{ primary: void; secondary: void }>();

	const iconMap: { [Key in MessageStyle]: keyof typeof iconsJson } = {
		neutral: 'info',
		pop: 'info',
		warn: 'warning',
		error: 'error'
	};

	const iconColorMap: { [Key in MessageStyle]: IconColor } = {
		neutral: 'pop',
		pop: 'pop',
		warn: 'warn',
		error: 'error'
	};

	const primaryButtonMap: { [Key in MessageStyle]: ButtonColor } = {
		neutral: 'primary',
		pop: 'primary',
		warn: 'warn',
		error: 'error'
	};
</script>

<div
	class="info-message"
	class:neutral={style == 'neutral'}
	class:error={style == 'error'}
	class:pop={style == 'pop'}
	class:warn={style == 'warn'}
	class:shadow
>
	<Icon name={iconMap[style]} color={iconColorMap[style]} />
	<div class="info-message__inner">
		<div class="info-message__content">
			{#if title}
				<div class="info-message__title text-base-13 text-semibold">{title}</div>
			{/if}
			<div class="info-message__text text-base-body-12"><slot /></div>
		</div>
		{#if primary || secondary}
			<div class="info-message__actions">
				{#if secondary}
					<Button color="neutral" kind="outlined" on:click={() => dispatch('secondary')}>
						{secondary}
					</Button>
				{/if}
				{#if primary}
					<Button color={primaryButtonMap[style]} on:click={() => dispatch('primary')}>
						{primary}
					</Button>
				{/if}
			</div>
		{/if}
	</div>
</div>

<style lang="postcss">
	.info-message {
		color: var(--clr-theme-scale-ntrl-0);
		display: flex;
		padding: var(--space-16);
		border-radius: var(--radius-m);
		gap: var(--space-12);
	}
	.info-message__inner {
		display: flex;
		flex-grow: 1;
		flex-direction: column;
		gap: var(--space-12);
		overflow-x: hidden;
	}
	.info-message__content {
		display: flex;
		flex-direction: column;
		gap: var(--space-8);
	}
	.info-message__actions {
		display: flex;
		gap: var(--space-6);
		justify-content: flex-end;
	}
	.neutral {
		background-color: var(--clr-theme-container-light);
		border: 1px solid var(--clr-theme-container-outline-light);
	}
	.error {
		background-color: var(--clr-theme-container-light);
		border: 1px solid var(--clr-theme-scale-err-60);
	}
	.pop {
		background-color: var(--clr-theme-container-light);
		border: 1px solid var(--clr-theme-scale-pop-50);
	}
	.warn {
		background-color: var(--clr-theme-container-light);
		border: 1px solid var(--clr-theme-scale-warn-60);
	}
	.shadow {
		box-shadow: 0px 7px 14px 0px rgba(0, 0, 0, 0.1);
	}

	/* rendered markdown requires global */
	:global(.info-message__text a) {
		cursor: pointer;
		text-decoration: underline;
		word-break: break-all; /* allow long links to wrap */
	}
	:global(.info-message__text p:not(:last-child)) {
		margin-bottom: var(--space-10);
	}
</style>
