<script lang="ts">
	import Icon from '$lib/icons/Icon.svelte';
	import { clickOutside } from '$lib/clickOutside';

	export let type: 'filled' | 'outlined' = 'filled';
	export let disabled = false;
	export let loading = false;
	let visible = false;

	export function show() {
		visible = true;
	}

	export function close() {
		visible = false;
	}

	let container: HTMLDivElement;
	let popup: HTMLDivElement;
	let iconElt: HTMLElement;
</script>

<div class="wrapper">
	<div
		class="dropdown"
		bind:this={container}
		class:filled={type == 'filled'}
		class:outlined={type == 'outlined'}
	>
		<button class="btn" disabled={disabled || loading} on:click>
			<span class="label text-base-12"> <slot /></span>
		</button>
		<button
			class="icon"
			bind:this={iconElt}
			disabled={disabled || loading}
			on:click={() => (visible = !visible)}
		>
			<Icon name={loading ? 'spinner' : visible ? 'chevron-top' : 'chevron-down'} />
		</button>
	</div>
	<div
		class="context-wrapper"
		use:clickOutside={{ trigger: iconElt, handler: () => (visible = !visible), enabled: visible }}
		bind:this={popup}
		style:display={visible ? 'block' : 'none'}
	>
		<slot name="popup" />
	</div>
</div>

<style lang="postcss">
	.wrapper {
		display: flex;
		position: relative;
	}

	.dropdown {
		display: flex;
		align-items: center;
	}

	.btn,
	.icon {
		display: flex;
		align-items: center;
		height: 100%;
		padding: var(--space-4) var(--space-6);
		&:disabled {
			opacity: 0.6;
		}
	}

	.label {
		display: inline-flex;
		padding: 0 var(--space-2);
	}

	.btn {
		border-radius: var(--radius-m) 0 0 var(--radius-m);
	}

	.icon {
		border-radius: 0 var(--radius-m) var(--radius-m) 0;
	}

	.filled {
		color: var(--clr-theme-pop-on-element);

		& .label:hover:not(:disabled),
		.icon:hover:not(:disabled) {
			background: var(--clr-theme-pop-element-dim);
		}

		& .label {
			background: var(--clr-theme-pop-element);
		}

		& .icon {
			background: var(--clr-theme-pop-element);
			border-left: 1px solid var(--clr-core-pop-55);
		}
	}

	.outlined:not(:hover) .icon {
		border-left: 1px solid var(--clr-theme-pop-outline);
	}

	.outlined {
		color: var(--clr-theme-pop-outline);
		border-color: (--clr-theme-pop-outline);

		& .label:hover:not(:disabled),
		.icon:hover:not(:disabled) {
			color: var(--clr-theme-pop-outline-dim);
			border-color: var(--clr-theme-pop-outline-dim);
		}

		& .btn {
			border-width: 1px 0 1px 1px;
			&:hover {
				border-right-width: 1px;
			}
		}

		& .icon {
			border-width: 1px 1px 1px 0;
			&:hover {
				border-left-width: 1px;
			}
		}
	}

	.context-wrapper {
		position: absolute;
		right: 0;
		bottom: 100%;
		padding-bottom: var(--space-4);
	}
</style>
