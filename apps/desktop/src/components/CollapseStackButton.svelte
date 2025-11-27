<script lang="ts">
	import { persisted } from '@gitbutler/shared/persisted';
	import { Tooltip } from '@gitbutler/ui';

	interface Props {
		stackId?: string;
		projectId: string;
		disabled?: boolean;
		isFolded?: boolean;
	}

	const { stackId, projectId, disabled, isFolded }: Props = $props();

	// Persisted folded stacks state per project (without expiration)
	const foldedStacks = persisted<string[]>([], `folded-stacks-${projectId}`);

	function toggleFold() {
		if (!stackId || disabled) return;

		const currentFolded = $foldedStacks;
		if (isFolded) {
			// Unfold: remove from folded list
			foldedStacks.set(currentFolded.filter((id) => id !== stackId));
		} else {
			// Fold: add to folded list
			if (!currentFolded.includes(stackId)) {
				foldedStacks.set([...currentFolded, stackId]);
			}
		}
	}
</script>

<Tooltip text={isFolded ? 'Expand stack' : 'Collapse stack'}>
	<button
		class="collapse-button"
		class:isFolded
		type="button"
		aria-label="Collapse stack"
		onclick={toggleFold}
		{disabled}
	>
		<svg class="collapse-icon" viewBox="0 0 15 10" fill="none" xmlns="http://www.w3.org/2000/svg">
			<path
				d="M11.75 0.75H2.75C1.64543 0.75 0.75 1.64543 0.75 2.75V6.75C0.75 7.85457 1.64543 8.75 2.75 8.75H11.75C12.8546 8.75 13.75 7.85457 13.75 6.75V2.75C13.75 1.64543 12.8546 0.75 11.75 0.75Z"
				stroke="currentColor"
				stroke-width="1.5"
				stroke-linecap="round"
			/>
			<rect
				class="collapse-icon__lane"
				x="0.75"
				y="0.75"
				width="5"
				height="8"
				rx="2"
				stroke="currentColor"
				stroke-width="1.5"
			/>
		</svg>
	</button>
</Tooltip>

<style lang="postcss">
	.collapse-button {
		display: flex;
		position: relative;
		align-items: center;
		justify-content: center;
		width: 24px;
		height: 24px;
		transition: color var(--transition-fast);

		&:hover {
			color: var(--clr-text-2);
		}

		&:not(.isFolded):hover {
			& .collapse-icon__lane {
				fill: currentColor;
			}
		}

		&:disabled {
			opacity: 0.5;
			pointer-events: none;
		}

		&.isFolded {
			.collapse-icon__lane {
				fill: currentColor;
			}
		}

		&.isFolded:hover {
			cursor: pointer;
			.collapse-icon__lane {
				fill: none;
			}
		}

		&:after {
			position: absolute;
			top: 50%;
			left: 50%;
			width: 18px;
			height: 14px;
			transform: translate(-50%, -50%);
			border-radius: var(--radius-m);
			background-color: var(--clr-bg-2);
			content: '';
		}
	}

	.collapse-icon {
		z-index: var(--z-ground);
		position: relative;
		width: 15px;
		height: 10px;
		--line-width: 0.094rem;
		--border-radius: 3px;
		cursor: pointer;
	}
</style>
