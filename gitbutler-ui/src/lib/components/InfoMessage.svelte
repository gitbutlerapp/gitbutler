<script lang="ts" context="module">
	export type MessageStyle = 'neutral' | 'error' | 'pop' | 'warn';
</script>

<script lang="ts">
	import Icon, { type IconColor } from '$lib/icons/Icon.svelte';
	import { createEventDispatcher } from 'svelte';
	import type iconsJson from '../icons/icons.json';
	import Button, { type ButtonColor } from './Button.svelte';

	export let style: MessageStyle = 'neutral';
	export let title: string;
	export let primary: string | undefined = undefined;
	export let secondary: string | undefined = undefined;

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

	const secondaryButtonMap: { [Key in MessageStyle]: ButtonColor } = {
		neutral: 'neutral',
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
>
	<Icon name={iconMap[style]} color={iconColorMap[style]} />
	<div class="info-message__inner">
		<div class="info-message__content">
			<div class="info-message__title text-base-13 text-semibold">{title}</div>
			<div class="info-message__text text-base-12">
				<slot />
			</div>
		</div>
		<div class="info-message__actions">
			{#if secondary}
				<Button
					color={secondaryButtonMap[style]}
					kind="outlined"
					on:click={() => dispatch('secondary')}
				>
					{secondary}
				</Button>
			{/if}
			{#if primary}
				<Button color={primaryButtonMap[style]} on:click={() => dispatch('primary')}>
					{primary}
				</Button>
			{/if}
		</div>
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
		background-color: var(--clr-theme-err-container);
		border: 1px solid var(--clr-theme-scale-err-70);
	}
	.pop {
		background-color: var(--clr-theme-pop-container);
		border: 1px solid var(--clr-theme-scale-pop-60);
	}
	.warn {
		background-color: var(--clr-theme-warn-container);
		border: 1px solid var(--clr-theme-scale-warn-70);
	}
</style>
