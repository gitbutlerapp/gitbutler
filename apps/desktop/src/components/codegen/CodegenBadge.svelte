<script lang="ts">
	import { Icon, Tooltip } from '@gitbutler/ui';

	interface Props {
		state: 'ebabled' | 'cli' | 'loading' | 'error';
		onclick?: () => void;
	}

	const { state, onclick }: Props = $props();

	function getTooltipText(state: string) {
		switch (state) {
			case 'ebabled':
				return 'Click to go to the session';
			case 'cli':
				return 'Session is running in CLI mode';
			case 'loading':
				return 'Session is loading';
			case 'error':
				return 'Error occurred in the session';
			default:
				return 'Click to go to the session';
		}
	}
</script>

<Tooltip text={getTooltipText(state)}>
	<button
		type="button"
		class="codegenbadge"
		class:enabled={state === 'ebabled' || state === 'cli' || state === 'loading'}
		disabled={state === 'loading' || state === 'error' || state === 'cli'}
		class:error={state === 'error'}
		{onclick}
	>
		<Icon name="ai-small" opacity={0.9} />
		{#if state === 'cli'}
			<span class="text-12 text-semibold">CLI</span>
		{/if}
		<div class="codegenbadge__drag-handle">
			<Icon name={state === 'loading' ? 'spinner' : 'draggable'} opacity={0.8} />
		</div>
	</button>
</Tooltip>

<style lang="scss">
	:global(:root) {
		--codegen-gradient: linear-gradient(135deg, #7c7afc 10%, #44cebb 50%, #7c7afc 100%);
		--codegen-color: var(--clr-theme-ntrl-on-element);
	}

	.codegenbadge {
		display: flex;
		align-items: center;
		height: var(--size-tag);
		padding: 0 4px;
		gap: 2px;
		border-radius: var(--radius-m);

		&.enabled {
			background: var(--codegen-gradient);
			background-position: 0% 50%;
			background-size: 200% 200%;
			color: var(--codegen-color);
			transition: background-position 0.3s ease-in-out;

			&:hover {
				background-position: 100% 50%;
			}
		}

		&.error {
			background-color: var(--clr-theme-err-element);
			color: var(--clr-theme-err-on-element);
		}

		&:disabled {
			cursor: not-allowed;
		}
	}

	.codegenbadge__drag-handle {
		display: flex;
		cursor: grab;
	}
</style>
