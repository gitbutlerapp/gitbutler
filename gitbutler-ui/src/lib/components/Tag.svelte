<script lang="ts" context="module">
	export type TagColor =
		| 'success'
		| 'error'
		| 'warning'
		| 'ghost'
		| 'light'
		| 'dark'
		| 'pop'
		| 'purple';
</script>

<script lang="ts">
	import Icon from '$lib/components/Icon.svelte';
	import { tooltip } from '$lib/utils/tooltip';
	import type iconsJson from '$lib/icons/icons.json';

	export let help = '';
	export let icon: keyof typeof iconsJson | undefined = undefined;
	export let reversedDirection = false;
	export let color: TagColor = 'light';
	export let border = false;
	export let filled = false;
	export let disabled = false;
	export let clickable = false;
	export let shrinkable = false;
	export let verticalOrientation = false;
</script>

<div
	class="tag text-base-11 text-semibold"
	class:ghost={color == 'ghost'}
	class:light={color == 'light'}
	class:dark={color == 'dark'}
	class:success={color == 'success'}
	class:error={color == 'error'}
	class:warning={color == 'warning'}
	class:purple={color == 'purple'}
	class:pop={color == 'pop'}
	class:tag-border={border}
	class:filled
	class:disabled
	class:shrinkable
	class:iconLeft={reversedDirection}
	class:verticalOrientation
	class:not-button={!clickable}
	on:click
	on:mousedown
	on:contextmenu
	role={clickable ? 'button' : undefined}
	class:clickable
	use:tooltip={help}
>
	<span class="label" class:verticalLabel={verticalOrientation}>
		<slot />
	</span>
	{#if icon}
		<div class="icon" class:verticalIcon={verticalOrientation}>
			<Icon name={icon} spinnerRadius={3.5} />
		</div>
	{/if}
</div>

<style lang="postcss">
	.tag {
		cursor: default;
		display: flex;
		align-items: center;
		justify-content: center;
		height: var(--size-control-s);
		padding: var(--size-2) var(--size-4);
		border-radius: var(--radius-m);
		transition: background-color var(--transition-fast);
	}
	.icon {
		flex-shrink: 0;
	}
	.label {
		white-space: nowrap;
		display: inline-block;
		padding: 0 var(--size-2);
	}
	.clickable {
		cursor: pointer;
	}

	/* colors */

	.ghost {
		color: var(--clr-theme-scale-ntrl-40);
		&:not(.not-button):hover {
			background: color-mix(in srgb, var(--clr-core-ntrl-50), transparent 90%);
		}
		&.tag-border {
			box-shadow: inset 0 0 0 1px var(--clr-theme-scale-ntrl-60);
		}
	}

	.light {
		color: var(--clr-theme-scale-ntrl-40);
		background: color-mix(in srgb, var(--clr-core-ntrl-50), transparent 85%);
		&:not(.not-button):hover {
			background: color-mix(in srgb, var(--clr-core-ntrl-50), transparent 75%);
		}
		&.tag-border {
			box-shadow: inset 0 0 0 1px var(--clr-theme-scale-ntrl-60);
		}
	}

	.dark {
		color: var(--clr-theme-scale-ntrl-100);
		background: var(--clr-theme-scale-ntrl-40);
		&:not(.not-button):hover {
			background: var(--clr-theme-scale-ntrl-30);
		}
	}

	.success {
		color: var(--clr-theme-scale-succ-20);
		background: color-mix(in srgb, var(--clr-core-succ-50), transparent 80%);
		&:not(.not-button):hover {
			background: color-mix(in srgb, var(--clr-core-succ-50), transparent 70%);
		}
		&.tag-border {
			box-shadow: inset 0 0 0 1px var(--clr-theme-scale-succ-60);
		}
		&.filled {
			color: var(--clr-theme-succ-on-element);
			background: var(--clr-theme-succ-element);
			&:not(.not-button):hover {
				background: color-mix(in srgb, var(--clr-theme-succ-element), var(--darken-mid));
			}
		}
	}

	.error {
		color: var(--clr-theme-scale-err-20);
		background: color-mix(in srgb, var(--clr-core-err-50), transparent 80%);
		&:not(.not-button):hover {
			background: color-mix(in srgb, var(--clr-core-err-50), transparent 70%);
		}
		&.tag-border {
			box-shadow: inset 0 0 0 1px var(--clr-theme-scale-err-60);
		}
		&.filled {
			color: var(--clr-theme-err-on-element);
			background: var(--clr-theme-err-element);
			&:not(.not-button):hover {
				background: color-mix(in srgb, var(--clr-theme-err-element), var(--darken-mid));
			}
		}
	}

	.warning {
		color: var(--clr-theme-scale-warn-20);
		background: color-mix(in srgb, var(--clr-core-warn-50), transparent 80%);
		&:not(.not-button):hover {
			background: color-mix(in srgb, var(--clr-core-warn-50), transparent 70%);
		}
		&.tag-border {
			box-shadow: inset 0 0 0 1px var(--clr-theme-scale-warn-60);
		}
		&.filled {
			color: var(--clr-theme-warn-on-element);
			background: var(--clr-theme-warn-element);
			&:not(.not-button):hover {
				background: color-mix(in srgb, var(--clr-theme-warn-element), var(--darken-mid));
			}
		}
	}

	.purple {
		color: var(--clr-theme-scale-purple-20);
		background: color-mix(in srgb, var(--clr-core-purple-50), transparent 80%);
		&:not(.not-button):hover {
			background: color-mix(in srgb, var(--clr-core-purple-50), transparent 70%);
		}
		&.tag-border {
			box-shadow: inset 0 0 0 1px var(--clr-theme-scale-purple-60);
		}
		&.filled {
			color: var(--clr-theme-purple-on-element);
			background: var(--clr-theme-purple-element);
			&:not(.not-button):hover {
				background: color-mix(in srgb, var(--clr-theme-purple-element), var(--darken-mid));
			}
		}
	}

	.pop {
		color: var(--clr-theme-scale-pop-20);
		background: color-mix(in srgb, var(--clr-core-pop-50), transparent 80%);
		&:not(.not-button):hover {
			background: color-mix(in srgb, var(--clr-core-pop-50), transparent 70%);
		}
		&.tag-border {
			box-shadow: inset 0 0 0 1px var(--clr-theme-scale-pop-60);
		}
		&.filled {
			color: var(--clr-theme-pop-on-element);
			background: var(--clr-theme-pop-element);
			&:not(.not-button):hover {
				background: color-mix(in srgb, var(--clr-theme-pop-element), var(--darken-mid));
			}
		}
	}

	/* modifiers */

	.not-button {
		user-select: none;
	}

	.disabled {
		background-color: color-mix(in srgb, var(--clr-theme-scale-ntrl-50) 10%, transparent);
		color: var(--clr-core-ntrl-50);
	}

	.iconLeft {
		flex-direction: row-reverse;
	}

	.shrinkable {
		overflow: hidden;

		& span {
			overflow: hidden;
			text-overflow: ellipsis;
		}
	}

	.verticalOrientation {
		writing-mode: vertical-rl;
		height: max-content;
		width: var(--size-control-s);
		padding: var(--size-4) var(--size-2);
		transform: rotate(180deg);
	}

	.verticalIcon {
		transform: rotate(90deg);
	}

	.verticalLabel {
		padding: var(--size-2) 0;
	}
</style>
