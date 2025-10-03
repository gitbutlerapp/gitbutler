<script lang="ts">
	import ArrowButton from '$home/components/ArrowButton.svelte';
	import SectionHeader from '$home/components/SectionHeader.svelte';
	import { onMount } from 'svelte';
	import quotesJson from '$home/data/social-quotes.json';

	type Quote = {
		social: string;
		handle: string;
		author: string;
		occupation: string;
		avatar: string;
		quote: string;
		source: string;
	};

	// Shuffle function using Fisher-Yates algorithm
	function shuffleArray<T>(array: T[]): T[] {
		const shuffled = [...array];
		for (let i = shuffled.length - 1; i > 0; i--) {
			const j = Math.floor(Math.random() * (i + 1));
			[shuffled[i], shuffled[j]] = [shuffled[j], shuffled[i]];
		}
		return shuffled;
	}

	// Randomize quotes before grouping into slides
	const shuffledQuotes = shuffleArray(quotesJson);

	// Detect mobile viewport
	let isMobile = false;

	// Group randomized quotes into slides based on viewport
	let QUOTES_PER_SLIDE = 4;
	let originalSlides: Quote[][] = [];
	let slides: Quote[][] = [];
	let currentSlide = 1;
	let isTransitioning = true;
	let totalOriginalSlides = 0;

	function createSlides() {
		QUOTES_PER_SLIDE = isMobile ? 3 : 4;
		originalSlides = [];

		for (let i = 0; i < shuffledQuotes.length; i += QUOTES_PER_SLIDE) {
			originalSlides.push(shuffledQuotes.slice(i, i + QUOTES_PER_SLIDE));
		}

		// Create infinite carousel by duplicating first and last slides
		slides = [
			originalSlides[originalSlides.length - 1], // Last slide at the beginning
			...originalSlides,
			originalSlides[0] // First slide at the end
		];

		totalOriginalSlides = originalSlides.length;
		currentSlide = 1; // Start at the first real slide (index 1)
	}

	// Mobile carousel element
	let mobileCarouselElement: HTMLElement;

	// Create a long list of quotes for mobile (repeat quotes multiple times for infinite feel)
	function createMobileQuotes() {
		const quotesPerSlide = 3;
		const repetitions = 5; // Repeat the quotes 5 times for infinite feel
		const mobileQuotes = [];

		for (let rep = 0; rep < repetitions; rep++) {
			for (let i = 0; i < shuffledQuotes.length; i += quotesPerSlide) {
				mobileQuotes.push(...shuffledQuotes.slice(i, i + quotesPerSlide));
			}
		}

		return mobileQuotes;
	}

	const mobileQuotes = createMobileQuotes();

	onMount(() => {
		// Check if mobile viewport
		function checkMobile() {
			isMobile = window.innerWidth < 768; // Adjust breakpoint as needed
			createSlides();
		}

		checkMobile();
		window.addEventListener('resize', checkMobile);

		return () => {
			window.removeEventListener('resize', checkMobile);
		};
	});

	// Initialize slides on component load
	createSlides();

	function nextSlide() {
		if (isMobile && mobileCarouselElement) {
			// Mobile: scroll to next quote - calculate actual quote width including gap
			const viewportWidth = window.innerWidth;
			const padding = 48; // 24px on each side
			const gap = 24; // gap between cards
			const maxCardWidth = 400;

			// Calculate effective card width: either viewport - padding or max width, whichever is smaller
			const cardWidth = Math.min(viewportWidth - padding, maxCardWidth);
			// Add gap to scroll to the next card properly
			const scrollAmount = cardWidth + gap;

			mobileCarouselElement.scrollBy({ left: scrollAmount, behavior: 'smooth' });
			return;
		}

		// Desktop: use existing infinite carousel logic
		if (!isTransitioning) return;

		currentSlide++;

		// If we've moved to the duplicate first slide at the end
		if (currentSlide === totalOriginalSlides + 1) {
			setTimeout(() => {
				isTransitioning = false;
				currentSlide = 1; // Reset to actual first slide
				setTimeout(() => {
					isTransitioning = true;
				}, 50);
			}, 400); // Match transition duration
		}
	}

	function prevSlide() {
		if (isMobile && mobileCarouselElement) {
			// Mobile: scroll to previous quote - calculate actual quote width including gap
			const viewportWidth = window.innerWidth;
			const padding = 48; // 24px on each side
			const gap = 24; // gap between cards
			const maxCardWidth = 400;

			// Calculate effective card width: either viewport - padding or max width, whichever is smaller
			const cardWidth = Math.min(viewportWidth - padding, maxCardWidth);
			// Add gap to scroll to the previous card properly
			const scrollAmount = cardWidth + gap;

			mobileCarouselElement.scrollBy({ left: -scrollAmount, behavior: 'smooth' });
			return;
		}

		// Desktop: use existing infinite carousel logic
		if (!isTransitioning) return;

		currentSlide--;

		// If we've moved to the duplicate last slide at the beginning
		if (currentSlide === 0) {
			setTimeout(() => {
				isTransitioning = false;
				currentSlide = totalOriginalSlides; // Reset to actual last slide
				setTimeout(() => {
					isTransitioning = true;
				}, 50);
			}, 400); // Match transition duration
		}
	}
</script>

<section class="social-quotes">
	<SectionHeader>
		<i>Community</i> voices

		{#snippet buttons()}
			<ArrowButton reverseDirection onclick={prevSlide} />
			<ArrowButton onclick={nextSlide} />
		{/snippet}
	</SectionHeader>

	<div class="carousel-container">
		<!-- Desktop carousel -->
		<div
			class="carousel-track desktop-only"
			class:transitioning={isTransitioning}
			style="transform: translateX(-{currentSlide * 100}%)"
		>
			{#each slides as slide}
				<div class="carousel-slide">
					{#each slide as quote}
						<blockquote class="quote">
							<p class="text-15 text-body quote__text">
								{quote.quote}
								<a
									title="View post on {quote.social}"
									class="quote__source"
									href={quote.source}
									target="_blank"
									rel="noopener noreferrer">[↗]</a
								>
							</p>
							<div class="quote__author-info">
								<img
									class="quote__author-avatar"
									src={quote.avatar}
									alt="image of {quote.author}"
								/>
								<div class="stack-v gap-4">
									<p class="text-15 text-bold quote__author">{quote.author}</p>
									<p class="text-13 quote__job-title">{quote.occupation}</p>
								</div>
							</div>
						</blockquote>
					{/each}
				</div>
			{/each}
		</div>

		<!-- Mobile scroll carousel -->
		<div class="mobile-carousel mobile-only" bind:this={mobileCarouselElement}>
			{#each mobileQuotes as quote, i}
				<blockquote class="quote mobile-quote" class:snap-start={i % 3 === 0}>
					<p class="text-15 text-body quote__text">
						{quote.quote}
						<a
							title="View post on {quote.social}"
							class="quote__source"
							href={quote.source}
							target="_blank"
							rel="noopener noreferrer">[↗]</a
						>
					</p>
					<div class="quote__author-info">
						<img class="quote__author-avatar" src={quote.avatar} alt="image of {quote.author}" />
						<div class="stack-v gap-4">
							<p class="text-15 text-bold quote__author">{quote.author}</p>
							<p class="text-13 quote__job-title">{quote.occupation}</p>
						</div>
					</div>
				</blockquote>
			{/each}
		</div>
	</div>
</section>

<style>
	.social-quotes {
		display: grid;
		grid-template-columns: subgrid;
		grid-column: full-start / full-end;
	}

	.carousel-container {
		display: flex;
		position: relative;
		grid-column: narrow-start / narrow-end;
		flex-direction: column;
		width: calc(100% + 80px);
		margin-left: -40px;
		overflow: hidden;

		&::after {
			z-index: 1;
			position: absolute;
			top: 0;
			left: 0;
			width: 40px;
			height: 100%;
			background: linear-gradient(to right, var(--clr-bg-2), transparent);
			content: '';
			pointer-events: none;
		}

		&::before {
			z-index: 1;
			position: absolute;
			top: 0;
			right: 0;
			width: 40px;
			height: 100%;
			background: linear-gradient(to left, var(--clr-bg-2), transparent);
			content: '';
			pointer-events: none;
		}
	}

	.carousel-track {
		display: flex;
	}

	.carousel-track.transitioning {
		transition: transform 0.4s cubic-bezier(0.4, 0, 0.2, 1);
	}

	.mobile-carousel {
		display: none;
		overflow-x: auto;
		scroll-behavior: smooth;
		scroll-snap-type: x mandatory;
		-webkit-overflow-scrolling: touch;
		scrollbar-width: none;
		-ms-overflow-style: none;
		grid-column: narrow-start / narrow-end;
		padding: 0 24px;
		gap: 24px;
		scroll-padding-left: 24px;
	}

	.mobile-carousel::-webkit-scrollbar {
		display: none;
	}

	.mobile-quote {
		flex: 0 0 calc(100vw - 48px);
		max-width: 400px;
		scroll-snap-align: start;
	}

	.snap-start {
		scroll-snap-align: start;
	}

	.desktop-only {
		display: flex;
	}

	.mobile-only {
		display: none;
	}

	.carousel-slide {
		display: grid;
		grid-template-columns: repeat(2, 1fr);
		column-gap: 40px;
		row-gap: 38px;
		flex: 0 0 100%;
		padding: 0 40px;
	}

	.quote {
		display: flex;
		flex-direction: column;
		margin: 0;
		gap: 20px;
		border: 1px solid var(--color-border-2);
		border-radius: 8px;
		background: var(--color-bg-2);
	}

	.quote__source {
		margin-left: 4px;
		color: var(--color-text-3);
		font-size: 12px;
		text-decoration: none;
	}

	.quote__source:hover {
		color: var(--color-text-1);
	}

	.quote__author-info {
		display: flex;
		align-items: center;
		gap: 12px;
	}

	.quote__author-avatar {
		width: 40px;
		height: 40px;
		object-fit: cover;
		border-radius: 16px;
	}

	.quote__job-title {
		color: var(--clr-text-2);
	}

	/* Mobile viewport */
	@media (--mobile-viewport) {
		.carousel-container {
			width: calc(100% + 48px);
			margin-left: -24px;
		}

		.desktop-only {
			display: none;
		}

		.mobile-only {
			display: flex;
		}

		.carousel-container::after,
		.carousel-container::before {
			display: none;
		}
	}
</style>
