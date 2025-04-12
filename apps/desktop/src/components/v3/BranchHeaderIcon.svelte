<script lang="ts">
	import Icon from '@gitbutler/ui/Icon.svelte';
	import type iconsJson from '@gitbutler/ui/data/icons.json';

	interface Props {
		iconName: keyof typeof iconsJson;
		lineColor: string;
		lineTop?: boolean;
		lineBottom?: boolean;
		isDashed?: boolean;
	}

	const { iconName, lineColor, lineTop = true, lineBottom = true, isDashed }: Props = $props();
</script>

<div class="stack__status gap">
	<div
		class="stack__status--bar"
		style:--bg-color={lineTop ? lineColor : 'var(--clr-transparent)'}
	></div>
	<div class="stack__status--icon" style:--bg-color={lineColor}>
		<Icon name={iconName} />
	</div>
	<div
		class="stack__status--bar last"
		class:dashed={isDashed}
		style:--bg-color={lineBottom ? lineColor : 'var(--clr-transparent)'}
	></div>
</div>

<style>
	.stack__status {
		flex: 0 0 auto;
		align-self: stretch;
		display: flex;
		flex-direction: column;
		justify-content: center;
		gap: 3px;
		width: 42px;
		--clr-transparent: transparent;

		& .stack__status--icon {
			display: flex;
			align-items: center;
			justify-content: center;
			width: 22px;
			height: 26px;
			border-radius: var(--radius-m);
			background-color: var(--bg-color);
			color: #fff;
			margin: 0 auto;
		}

		& .stack__status--bar {
			width: 2px;
			height: 8px;
			margin: 0 auto;
			background: var(--bg-color);

			&.dashed {
				background: repeating-linear-gradient(
					0deg,
					var(--bg-color),
					var(--bg-color) 2px,
					var(--clr-transparent) 2px,
					var(--clr-transparent) 4px
				);
			}

			&.last {
				flex: 1;
			}
		}
	}
</style>
