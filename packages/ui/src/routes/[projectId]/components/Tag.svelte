<script lang="ts" context="module">
	export type TagColor =
		| 'success'
		| 'error'
		| 'warning'
		| 'neutral-light'
		| 'neutral-dim'
		| 'ghost';
</script>

<script lang="ts">
	import Icon from '$lib/icons/Icon.svelte';
	import type iconsJson from '$lib/icons/icons.json';

	export let icon: keyof typeof iconsJson | undefined = undefined;
	export let color: TagColor;
	export let border = false;
	export let filled = false;
	export let clickable = false;
</script>

<div
	class="tag text-base-11 text-semibold"
	class:success={color == 'success'}
	class:error={color == 'error'}
	class:warning={color == 'warning'}
	class:neutral-light={color == 'neutral-light'}
	class:neutral-dim={color == 'neutral-dim'}
	class:ghost={color == 'ghost'}
	class:tag-border={border}
	class:filled
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
		cursor: pointer;
		&:hover {
			background: var(--clr-theme-container-sub);
		}
	}

	/* colors */

	.ghost {
		color: var(--clr-theme-scale-ntrl-40);
		&:hover {
			background: var(--clr-theme-container-sub);
		}
		&.tag-border {
			box-shadow: inset 0 0 0 1px var(--clr-theme-scale-ntrl-60);
		}
	}

	.success {
		color: var(--clr-theme-succ-outline-dark);
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
		color: var(--clr-theme-err-outline-dark);
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
		color: var(--clr-theme-warn-outline-dark);
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
		background: var(--clr-theme-container-mid);
		&:hover {
			background: var(--clr-theme-container-dim);
		}
		&.tag-border {
			box-shadow: inset 0 0 0 1px var(--clr-theme-scale-ntrl-60);
		}
		&.filled {
			background: var(--clr-theme-scale-ntrl-40);
			&:hover {
				background: var(--clr-theme-scale-ntrl-30);
			}
		}
	}

	.neutral-dim {
		color: var(--clr-theme-scale-ntrl-20);
		background: var(--clr-theme-container-dim);
		&:hover {
			background: var(--clr-theme-container-dark);
		}
		&.tag-border {
			box-shadow: inset 0 0 0 1px var(--clr-theme-scale-ntrl-40);
		}
		&.filled {
			background: var(--clr-theme-scale-ntrl-30);
			&:hover {
				background: var(--clr-theme-scale-ntrl-10);
			}
		}
	}

	/* modifiers */

	.not-button {
		pointer-events: none;
	}
</style>
