<script lang="ts">
	import Icon from '@gitbutler/ui/Icon.svelte';
	import type { Snippet } from 'svelte';

	interface Props {
		count: number;
		isLast?: boolean;
		alignTop?: boolean;
		displayHeader?: boolean;
		unfoldable?: boolean;
		type: 'upstream' | 'integrated';
		title: Snippet;
		children: Snippet;
	}

	const { count, isLast, unfoldable, type, displayHeader, alignTop, title, children }: Props =
		$props();

	let isOpen = $state(count === 1);

	function toggle() {
		isOpen = !isOpen;
	}
</script>

<div
	class="commits-accordion {type}"
	class:is-last={isLast}
	class:is-open={isOpen}
	class:align-elements-to-top={alignTop}
>
	{#if displayHeader}
		<button type="button" class="commits-accordion-row__header" onclick={toggle}>
			<div class="commits-accordion-row__line">
				{#if !isOpen && count !== 1}
					<div class="dots" style="--dots-count: {count}">
						{#each new Array(Math.min(count, 3)) as _}
							<svg
								viewBox="0 0 14 14"
								class="upstream-dot"
								fill="none"
								xmlns="http://www.w3.org/2000/svg"
							>
								<rect
									x="1.76782"
									y="1.76764"
									width="10.535"
									height="10.535"
									rx="3"
									stroke-width="2"
								/>
							</svg>
						{/each}
					</div>
				{/if}
			</div>
			<div class="commits-accordion-row__right">
				<div class="title">
					{@render title()}
				</div>
				{#if !unfoldable}
					<Icon name={isOpen ? 'chevron-up' : 'chevron-down'} />
				{/if}
			</div>
		</button>
	{/if}
	{#if isOpen}
		<div class="commits-accordion-children">
			{@render children()}
		</div>
	{/if}
</div>

<style lang="postcss">
	.commits-accordion {
		position: relative;
		display: flex;
		flex-direction: column;

		&:focus {
			outline: none;
		}

		&:last-child,
		&.is-last {
			border-bottom: none;
			border-radius: 0 0 var(--radius-m) var(--radius-m);
		}

		&.is-open {
			& .commits-accordion-row__header {
				border-bottom: 1px solid var(--clr-border-2);
			}
		}

		&:not(:last-child) {
			border-bottom: 1px solid var(--clr-border-2);
		}
	}

	.commits-accordion-row__header {
		display: flex;
		width: 100%;
		min-height: 44px;
		align-items: stretch;
		text-align: left;

		&:not(:last-child) {
			border-bottom: 1px solid var(--clr-border-2);
		}

		& .title {
			flex: 1;
			display: block;
			color: var(--clr-text-1);
			width: 100%;
		}
	}

	.commits-accordion-row__line {
		position: relative;
		width: 2px;
		margin: 0 22px 0 20px;
		--dots-y-shift: -8px;

		& .upstream-dot {
			width: 14px;
			height: 14px;
			margin-top: calc(var(--dots-y-shift) + 2px);

			rect {
				transform: rotate(45deg);
				transform-origin: center;
			}
		}

		& .dots {
			display: flex;
			flex-direction: column;
			position: absolute;
			top: calc(50% - var(--dots-y-shift) / var(--dots-count));
			left: 50%;
			transform: translate(-50%, -50%);
		}
	}

	.commits-accordion-row__right {
		display: flex;
		flex: 1;
		padding: 12px 14px 12px 0;
		align-items: center;
		color: var(--clr-text-2);
	}

	.commits-accordion-children {
		display: flex;
		flex-direction: column;
		width: 100%;
		min-height: 44px;
		align-items: stretch;
		text-align: left;
	}

	/* TYPE MODIFIERS */
	.commits-accordion {
		&.upstream {
			& .commits-accordion-row__header {
				background-color: var(--clr-theme-warn-bg);
			}

			& .commits-accordion-row__line {
				background-color: var(--clr-commit-upstream);

				& .upstream-dot {
					fill: var(--clr-commit-upstream);
					stroke: var(--clr-theme-warn-bg);
				}
			}
		}

		&.integrated {
			& .commits-accordion-row__header {
				background-color: var(--clr-theme-purp-bg);
			}

			& .commits-accordion-row__line {
				background-color: var(--clr-commit-integrated);

				& .upstream-dot {
					fill: var(--clr-commit-integrated);
					stroke: var(--clr-theme-purp-bg);
				}
			}
		}
	}

	/* ALIGN ELEMENT TO TOP */
	/* for long description text
	we need to align the elements to the top */
	.commits-accordion {
		&.align-elements-to-top {
			& .commits-accordion-row__line .dots {
				top: 28px;
			}

			& .commits-accordion-row__right {
				align-items: flex-start;
			}
		}
	}
</style>
