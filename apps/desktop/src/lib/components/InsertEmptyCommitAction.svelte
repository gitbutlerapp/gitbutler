<script lang="ts">
	import { stackingFeature } from '$lib/config/uiFeatureFlags';
	import Button from '@gitbutler/ui/Button.svelte';

	interface Props {
		isLast?: boolean;
		isFirst?: boolean;
		onclick?: () => void;
	}

	const { isLast = false, isFirst = false, onclick }: Props = $props();
</script>

<div
	class="line-container"
	class:is-last={isLast}
	class:is-first={isFirst}
	class:not-stacking={!$stackingFeature}
>
	<div class="barnch-plus-btn">
		<Button
			style="ghost"
			outline
			solidBackground
			icon="plus-small"
			size="tag"
			width={26}
			tooltip="Insert empty commit"
			helpShowDelay={500}
			onclick={onclick?.()}
		/>
	</div>
</div>

<style lang="postcss">
	.line-container {
		--height: 14px;
		--container-margin: calc(var(--height) / 2 * -1);

		position: relative;
		display: flex;
		/* justify-content: center; */
		width: 100%;
		height: var(--height);
		z-index: var(--z-lifted);
		margin-top: var(--container-margin);
		margin-bottom: var(--container-margin);

		&:hover {
			& .barnch-plus-btn {
				pointer-events: all;
				transform: translateY(-50%);
				opacity: 1;
			}
		}

		&:not(.is-last, .is-first) {
			&:before {
				pointer-events: none;
				content: '';
				position: absolute;
				top: 50%;
				left: 0;
				width: 100%;
				height: 1px;
				background-color: var(--clr-border-2);
				transform: translateY(-50%);
				opacity: 0;
				transition: opacity var(--transition-fast);
			}

			&:hover {
				&:before {
					opacity: 1;
				}
			}
		}
	}

	.barnch-plus-btn {
		position: absolute;
		right: 14px;
		top: 50%;
		width: fit-content;
		display: flex;
		align-items: center;
		transform: translateY(-40%);
		opacity: 0;
		pointer-events: none;
		transition:
			opacity var(--transition-fast),
			transform var(--transition-medium);
	}

	/* MODIFIERS */
	.line-container.not-stacking.is-first {
		transform: translateY(16px);
	}

	.line-container.not-stacking.is-last {
		transform: translateY(-4px);
	}
</style>
