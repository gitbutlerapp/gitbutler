<script lang="ts" context="module">
	export type TagColor = 'success' | 'error' | 'warning' | 'neutral-light' | 'ghost';
</script>

<script lang="ts">
	import Icon from '$lib/icons/Icon.svelte';
	import type iconsJson from '$lib/icons/icons.json';

	export let icon: keyof typeof iconsJson | undefined = undefined;
	export let color: TagColor = 'neutral-light';
	export let border = false;
	export let filled = false;
	export let disabled = false;
	export let clickable = false;
</script>

<div
	class="tag text-base-11 text-semibold"
	class:success={color == 'success'}
	class:error={color == 'error'}
	class:warning={color == 'warning'}
	class:neutral-light={color == 'neutral-light'}
	class:ghost={color == 'ghost'}
	class:tag-border={border}
	class:filled
	class:disabled
	class:not-button={!clickable}
	on:click
	role={clickable ? 'button' : undefined}
	class:clickable
>
	<span class="label">
		<slot />
	</span>
	{#if icon}
		<div class="icon">
			<Icon name={icon} />
		</div>
	{/if}
</div>

<style lang="postcss">
	.tag {
		cursor: default;
		display: flex;
		align-items: center;
		justify-content: center;
		height: var(--size-btn-s);
		padding: var(--space-2) var(--space-4);
		border-radius: var(--radius-m);
		transition: background-color var(--transition-fast);
	}
	.icon {
		flex-shrink: 0;
	}
	.label {
		display: inline-block;
		padding: 0 var(--space-2);
	}
	.clickable {
		cursor: default;
		&:hover {
			background: var(--clr-theme-container-sub);
		}
	}

	/* colors */

	.ghost {
		color: var(--clr-theme-scale-ntrl-40);
		&:hover {
			background: var(--clr-theme-container-pale);
		}
		&.tag-border {
			box-shadow: inset 0 0 0 1px var(--clr-theme-scale-ntrl-60);
		}
	}

	.success {
		color: var(--clr-theme-scale-succ-30);
		background: var(--clr-theme-succ-container);
		&:hover {
			background: var(--clr-theme-succ-container-dim);
		}
		&.tag-border {
			box-shadow: inset 0 0 0 1px var(--clr-theme-scale-succ-60);
		}
		&.filled {
			color: var(--clr-theme-succ-on-element);
			background: var(--clr-theme-succ-element);
			&:hover {
				background: var(--clr-theme-succ-element-dim);
			}
		}
	}

	.error {
		color: var(--clr-theme-scale-err-30);
		background: var(--clr-theme-err-container);
		&:hover {
			background: var(--clr-theme-err-container-dim);
		}
		&.tag-border {
			box-shadow: inset 0 0 0 1px var(--clr-theme-scale-err-60);
		}
		&.filled {
			color: var(--clr-theme-err-on-element);
			background: var(--clr-theme-err-element);
			&:hover {
				background: var(--clr-theme-err-element-dim);
			}
		}
	}

	.warning {
		color: var(--clr-theme-scale-warn-30);
		background: var(--clr-theme-warn-container);
		&:hover {
			background: var(--clr-theme-warn-container-dim);
		}
		&.tag-border {
			box-shadow: inset 0 0 0 1px var(--clr-theme-scale-warn-60);
		}
		&.filled {
			color: var(--clr-theme-warn-on-element);
			background: var(--clr-theme-warn-element);
			&:hover {
				background: var(--clr-theme-warn-element-dim);
			}
		}
	}

	.neutral-light {
		color: var(--clr-theme-scale-ntrl-40);
		background: var(--clr-theme-container-pale);
		&:hover {
			background: var(--clr-theme-container-sub);
		}
		&.tag-border {
			box-shadow: inset 0 0 0 1px var(--clr-theme-scale-ntrl-60);
		}
	}

	/* modifiers */

	.not-button {
		pointer-events: none;
	}

	.disabled {
		pointer-events: none;
		background-color: color-mix(in srgb, var(--clr-theme-scale-ntrl-50) 10%, transparent);
		color: var(--clr-core-ntrl-50);
	}
</style>
