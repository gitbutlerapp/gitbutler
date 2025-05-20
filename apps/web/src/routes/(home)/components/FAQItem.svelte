<script lang="ts">
	import { slide } from 'svelte/transition';

	interface Props {
		faqItem: {
			label: string;
			content: string;
			bgIndex: string;
		};
	}

	let { faqItem }: Props = $props();

	let isOpen = $state(false);
</script>

<article class="faq-item">
	<div
		role="button"
		tabindex="0"
		class="faq-item__header"
		style="background-image: url('/images/patterns/faq-{faqItem.bgIndex}.gif')"
		onkeydown={(e) => {
			if (e.key === 'Enter') {
				isOpen = !isOpen;
			}
		}}
		onclick={() => {
			isOpen = !isOpen;
		}}
	>
		<h3 class="faq-item__title">{faqItem.label}</h3>
		<div class="faq-item__plus" class:show-minus={isOpen}></div>
	</div>
	{#if isOpen}
		<div class="faq-item__content" transition:slide={{ duration: 150 }}>
			<p>{faqItem.content}</p>
		</div>
	{/if}
</article>

<style lang="scss">
	.faq-item {
		display: flex;
		flex-direction: column;
		overflow: hidden;
		border-radius: 20px;
	}

	.faq-item__header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 28px 20px;
		gap: 20px;
		background-size: 14px;
		cursor: pointer;
		transition: filter 0.1s ease-in-out;
		user-select: none;

		&:hover {
			filter: brightness(0.9);
		}
	}

	.faq-item__title {
		flex: 1;
		color: var(--clr-black);
		font-weight: 500;
		font-size: 24px;
		line-height: 110%;
		text-transform: uppercase;

		@media (max-width: 800px) {
			font-size: 20px;
		}
	}

	.faq-item__plus {
		position: relative;
		flex-shrink: 0;
		width: 24px;
		height: 24px;

		&::before,
		&::after {
			position: absolute;
			top: 50%;
			left: 50%;
			width: 100%;
			height: 2px;
			transform: translate(-50%, -50%);
			background-color: var(--clr-black);
			content: '';
			transition: transform 0.1s ease-in-out;
		}

		&::after {
			transform: translate(-50%, -50%) rotate(90deg);
		}

		&::before {
			transform: translate(-50%, -50%);
		}
	}

	.show-minus {
		&:after {
			transform: translate(-50%, -50%) rotate(90deg) scaleX(0);
		}
	}

	.faq-item__content {
		padding: 40px;
		background-color: var(--clr-white);

		p {
			color: var(--clr-black);
			font-size: 18px;
			line-height: 140%;
		}

		@media (max-width: 800px) {
			padding: 20px;
		}
	}
</style>
