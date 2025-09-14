<script lang="ts">
	import loadErrorSvg from '$lib/assets/empty-state/load-error.svg?raw';
	import { Icon, Button } from '@gitbutler/ui';

	interface Props {
		onSettingsBtnClick: () => void;
	}

	const { onSettingsBtnClick }: Props = $props();
</script>

<section class="not-available-banner__wrap">
	<div class="not-available-banner">
		<div class="not-available-banner__image">
			{@html loadErrorSvg}
		</div>
		<div class="not-available-banner__content">
			<h3 class="text-16 text-semibold">
				<span class="not-available-banner__icon">
					<Icon name="warning" />
				</span> Claude code can't be found
			</h3>
			<p class="text-13 text-body clr-text-2 m-bottom-8">
				Connect Claude Code to keep going with this session.
				<br />
				This can be configured in the settings.
			</p>
			<Button kind="outline" style="neutral" icon="mixer" type="button" onclick={onSettingsBtnClick}
				>Configure Claude Code…</Button
			>
		</div>
	</div>
</section>

<style lang="postcss">
	.not-available-banner__wrap {
		display: flex;
		position: relative;
		padding: 12px;
		padding-top: 0;

		&::before {
			position: absolute;
			bottom: 0;
			left: 0;
			width: 100%;
			height: calc(100% + 60px);
			background: linear-gradient(0deg, var(--clr-bg-1-muted) 70%, transparent 100%);
			content: '';
		}
	}

	.not-available-banner {
		display: flex;
		z-index: var(--z-ground);
		align-items: center;
		width: 100%;
		padding: 28px;
		overflow: hidden;
		gap: 28px;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml);
		background-color: var(--clr-bg-1);
	}

	.not-available-banner__image {
		flex-shrink: 0;
	}

	.not-available-banner__content {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: 10px;
	}

	.not-available-banner__icon {
		display: inline-flex;
		margin-right: 2px;
		transform: translateY(2px);
		color: var(--clr-theme-warn-element);
	}

	@container chat (min-width: 800px) {
		.not-available-banner__wrap {
			position: absolute;
			bottom: 0;
			left: 0;
			justify-content: center;
			width: 100%;

			&::before {
				display: none;
			}
		}

		.not-available-banner {
			width: auto;
			max-width: 560px;
		}
	}
</style>
