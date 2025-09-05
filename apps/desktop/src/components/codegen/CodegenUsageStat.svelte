<script lang="ts">
	type Props = {
		label: string;
		value: string;
		valueSize?: 'small' | 'large';
	};

	const { label, value, valueSize = 'small' }: Props = $props();
</script>

<div class="usage-stat">
	<p class="text-12 clr-text-2">{label}</p>
	<div class="usage-stat-spacer"></div>
	<p
		class:text-12={valueSize === 'small'}
		class:text-13={valueSize === 'large'}
		class:text-bold={valueSize === 'large'}
	>
		{value}
	</p>
</div>

<style lang="postcss">
	.usage-stat {
		display: flex;
		width: 100%;
		gap: 8px;
	}

	.usage-stat-spacer {
		flex-grow: 1;

		border-bottom: 1px dotted var(--clr-text-3);
	}
</style>
