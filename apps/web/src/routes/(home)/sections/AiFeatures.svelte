<script lang="ts">
	import Features from '$home/components/Features.svelte';
	import SectionHeader from '$home/components/SectionHeader.svelte';
	import contentJSON from '$home/data/content.json';

	const aiFetures = contentJSON['ai-features'];

	let isPlaying = $state(false);

	function playVideo() {
		isPlaying = true;
	}

	function handleKeydown(event: KeyboardEvent) {
		if (event.key === 'Enter' || event.key === ' ') {
			event.preventDefault();
			playVideo();
		}
	}
</script>

<section class="ai-features">
	<SectionHeader>
		<i>Orchestrate</i> your AI Tools ✨
	</SectionHeader>
	<div class="ai-features__video">
		{#if isPlaying}
			<iframe
				width="100%"
				height="100%"
				src={`${contentJSON['ai-fetures-demo']}&autoplay=1`}
				title="YouTube video player"
				frameborder="0"
				allow="accelerometer; autoplay; clipboard-write; encrypted-media; gyroscope; picture-in-picture; web-share"
				referrerpolicy="strict-origin-when-cross-origin"
				allowfullscreen
			></iframe>
		{:else}
			<div
				class="video-preview"
				role="button"
				tabindex="0"
				onclick={playVideo}
				onkeydown={handleKeydown}
			>
				<img src="images/ai-demo.png" alt="AI Features Demo" loading="lazy" />
				<div class="play-button">
					<svg
						width="70"
						height="50"
						viewBox="0 0 70 50"
						fill="none"
						xmlns="http://www.w3.org/2000/svg"
					>
						<path
							d="M68.5375 8.50243C67.7324 5.51706 65.3606 3.16596 62.3484 2.36806C56.8899 0.917969 35 0.917969 35 0.917969C35 0.917969 13.1104 0.917969 7.6516 2.36806C4.6394 3.16596 2.26705 5.51706 1.46255 8.50243C0 13.9135 0 25.2037 0 25.2037C0 25.2037 0 36.4933 1.46255 41.905C2.26705 44.8906 4.6394 47.2412 7.6513 48.0399C13.1101 49.4894 34.9997 49.4895 34.9997 49.4895C34.9997 49.4895 56.8896 49.4894 62.3481 48.0399C65.3603 47.2415 67.7321 44.8907 68.5372 41.9053C70 36.4936 70 25.204 70 25.204C70 25.204 70.0003 13.9135 68.5375 8.50243Z"
							fill="#FF0000"
						/>
						<path d="M27.1426 34.5937L45.438 24.1861L27.1426 13.7773V34.5937Z" fill="white" />
					</svg>
				</div>
			</div>
		{/if}
	</div>

	<Features items={aiFetures} />
</section>

<style>
	.ai-features {
		display: grid;
		grid-template-columns: subgrid;
		grid-column: full-start / full-end;
		border-radius: var(--radius-xl);
		background-color: var(--clr-bg-2);
	}

	.ai-features__video {
		position: relative;
		grid-column: narrow-start / narrow-end;
		aspect-ratio: 16 / 9;
		width: 100%;
		overflow: hidden;
		border-radius: var(--radius-xl);
		box-shadow: 0 4px 14px rgba(0, 0, 0, 0.1);
	}

	.video-preview {
		position: relative;
		width: 100%;
		height: 100%;
		cursor: pointer;
		transition: transform 0.2s ease;
	}

	.video-preview:hover {
		transform: scale(1.02);

		.play-button {
			transform: translate(-50%, -50%) scale(0.9);
		}
	}

	.video-preview img {
		width: 100%;
		height: 100%;
		object-fit: cover;
	}

	.play-button {
		display: flex;
		position: absolute;
		top: 50%;
		left: 50%;
		align-items: center;
		justify-content: center;
		transform: translate(-50%, -50%);
		pointer-events: none;
		transition: transform 0.2s ease;
	}
</style>
