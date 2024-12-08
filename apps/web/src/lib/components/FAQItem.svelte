<script lang="ts">
	import { slide } from 'svelte/transition';

	export let faqItem: {
		label: string;
		content: string;
		bgIndex: string;
	};

	let isOpen = false;
</script>

<article class="faq-item">
	<div
		role="button"
		tabindex="0"
		class="faq-item__header"
		style="background-image: url('/images/patterns/faq-{faqItem.bgIndex}.gif')"
		on:keydown={(e) => {
			if (e.key === 'Enter') {
				isOpen = !isOpen;
			}
		}}
		on:click={() => {
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
		border-radius: 20px;
		overflow: hidden;
	}

	.faq-item__header {
		user-select: none;
		display: flex;
		gap: 20px;
		align-items: center;
		justify-content: space-between;
		padding: 28px 20px;
		cursor: pointer;
		background-size: 14px;
		transition: filter 0.1s ease-in-out;

		&:hover {
			filter: brightness(0.9);
		}
	}

	.faq-item__title {
		flex: 1;
		font-weight: 500;
		font-size: 24px;
		text-transform: uppercase;
		color: var(--clr-black);
		line-height: 110%;

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
			content: '';
			position: absolute;
			top: 50%;
			left: 50%;
			width: 100%;
			height: 2px;
			background-color: var(--clr-black);
			transform: translate(-50%, -50%);
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
			font-size: 18px;
			line-height: 140%;
			color: var(--clr-black);
		}

		@media (max-width: 800px) {
			padding: 20px;
		}
	}
</style>
