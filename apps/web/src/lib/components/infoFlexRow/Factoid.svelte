<script lang="ts">
	import type { Snippet } from 'svelte';

	type Props = {
		label: string;
		placeholderText?: string;
		children: Snippet;
	};

	const { label, placeholderText, children }: Props = $props();
</script>

<div class="flexrow-item">
	<p class="text-12 flexrow-item__title">{label}:</p>

	<div class="text-12 flexrow-item__content">
		{@render children()}
	</div>

	<p class="text-12 truncate flexrow-item__placeholder">{placeholderText}</p>
</div>

<style lang="postcss">
	.flexrow-item {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}

	.flexrow-item__title {
		color: var(--clr-text-2);
	}

	.flexrow-item__content {
		display: flex;
		gap: 8px;
	}

	/* Show placeholder if empty */
	.flexrow-item__content:empty {
		display: none;

		& + .flexrow-item__placeholder {
			display: block;
		}
	}

	.flexrow-item__placeholder {
		display: none;
		color: var(--clr-text-3);
	}
</style>
