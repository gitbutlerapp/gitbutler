<script lang="ts" context="module">
	export type TagColor = 'success' | 'error' | 'warning' | 'neutral-light' | 'neutral-dim';
</script>

<script lang="ts">
	import Icon from '$lib/icons/Icon.svelte';
	import type iconsJson from '$lib/icons/icons.json';

	export let icon: keyof typeof iconsJson | undefined = undefined;
	export let color: TagColor;
	export let border = false;
	export let filled = false;
</script>

<div
	class="tag"
	class:success={color == 'success'}
	class:error={color == 'error'}
	class:warning={color == 'warning'}
	class:neutral-light={color == 'neutral-light'}
	class:neutral-dim={color == 'neutral-dim'}
	class:border
	class:filled
>
	<div class="label text-base-11">
		<slot />
	</div>
	<div class="icon">
		{#if icon}
			<Icon name={icon} />
		{/if}
	</div>
</div>

<style lang="postcss">
	.tag {
		display: flex;
		padding: var(--space-2) var(--space-4);
		border-radius: var(--radius-m);
		align-items: center;
	}
	.icon {
		flex-shrink: 0;
	}
	.success {
		color: var(--clr-theme-succ-outline-dark);
		background: var(--clr-theme-succ-container);
		&:hover {
			background: var(--clr-theme-succ-container-dim);
		}
		&.border {
			border: 1px solid var(--clr-theme-scale-succ-60);
		}
		&.filled {
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
		&.border {
			border: 1px solid var(--clr-theme-scale-err-60);
		}
		&.filled {
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
		&.border {
			border: 1px solid var(--clr-theme-scale-warn-60);
		}
		&.filled {
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
		&.border {
			border: 1px solid var(--clr-theme-scale-ntrl-60);
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
		&.border {
			border: 1px solid var(--clr-theme-scale-ntrl-60);
		}
		&.filled {
			background: var(--clr-theme-scale-ntrl-30);
			&:hover {
				background: var(--clr-theme-scale-ntrl-10);
			}
		}
	}
</style>
