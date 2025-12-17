<script lang="ts">
	import { onMount } from 'svelte';

	let canvas: HTMLCanvasElement;
	let ctx: CanvasRenderingContext2D;
	let animationFrame: number;

	interface Star {
		x: number;
		y: number;
		z: number;
		prevX?: number;
		prevY?: number;
	}

	let stars: Star[] = [];
	const numStars = 800;
	const baseSpeed = 0.05; // Base speed for gentle, meditative movement
	const boostSpeed = 0.8; // Anime-style hyperspeed on hover
	let currentSpeed = $state(baseSpeed);
	let targetSpeed = $state(baseSpeed);
	const maxDepth = 32;
	// Removed unused isHovered variable

	function initStars() {
		const dpr = window.devicePixelRatio || 1;
		const displayWidth = canvas.width / dpr;
		const displayHeight = canvas.height / dpr;

		stars = [];
		for (let i = 0; i < numStars; i++) {
			stars.push({
				x: Math.random() * displayWidth - displayWidth / 2,
				y: Math.random() * displayHeight - displayHeight / 2,
				z: Math.random() * maxDepth
			});
		}
	}

	function drawStars() {
		if (!ctx || !canvas) return;

		// Smoothly transition speed for anime effect
		currentSpeed += (targetSpeed - currentSpeed) * 0.1;

		// Get device pixel ratio and display dimensions
		const dpr = window.devicePixelRatio || 1;
		const displayWidth = canvas.width / dpr;
		const displayHeight = canvas.height / dpr;

		// Always clear canvas completely for full transparency
		ctx.clearRect(0, 0, displayWidth, displayHeight);

		const centerX = displayWidth / 2;
		const centerY = displayHeight / 2;

		// Calculate trail intensity based on speed
		const speedRatio = currentSpeed / baseSpeed;
		const trailLength = Math.min(speedRatio * 0.8, 0.9);

		stars.forEach((star) => {
			// Move star closer with dynamic speed
			star.z -= currentSpeed;

			// Reset star if it's too close
			if (star.z <= 0) {
				const dpr = window.devicePixelRatio || 1;
				const displayWidth = canvas.width / dpr;
				const displayHeight = canvas.height / dpr;

				star.x = Math.random() * displayWidth - displayWidth / 2;
				star.y = Math.random() * displayHeight - displayHeight / 2;
				star.z = maxDepth;
				star.prevX = undefined;
				star.prevY = undefined;
			}

			// Project 3D position to 2D
			const scale = maxDepth / star.z;
			const x = centerX + star.x * scale;
			const y = centerY + star.y * scale;

			// Calculate star size and brightness based on depth
			const size = (1 - star.z / maxDepth) * 2;
			const opacity = 1 - star.z / maxDepth;

			// Draw trail from previous position with dynamic length
			if (star.prevX !== undefined && star.prevY !== undefined) {
				ctx.beginPath();
				ctx.strokeStyle = `rgba(36, 180, 173, ${opacity * trailLength})`;
				ctx.lineWidth = size * (1 + speedRatio * 0.02); // Dynamic line width based on speed
				ctx.moveTo(star.prevX, star.prevY);
				ctx.lineTo(x, y);
				ctx.stroke();
			}

			// Draw star
			ctx.beginPath();
			ctx.fillStyle = `rgba(36, 180, 173, ${opacity})`;
			ctx.arc(x, y, size, 0, Math.PI * 2);
			ctx.fill();

			// Store current position for next frame
			star.prevX = x;
			star.prevY = y;
		});

		animationFrame = requestAnimationFrame(drawStars);
	}

	function handleResize() {
		if (!canvas) return;

		// Get device pixel ratio for retina display support
		const dpr = window.devicePixelRatio || 1;

		// Set display size (css pixels)
		const rect = canvas.getBoundingClientRect();

		// Set actual size in memory (scaled for retina)
		canvas.width = rect.width * dpr;
		canvas.height = rect.height * dpr;

		// Scale canvas context to match device pixel ratio
		ctx.scale(dpr, dpr);

		initStars();
	}

	onMount(() => {
		ctx = canvas.getContext('2d')!;
		handleResize();

		window.addEventListener('resize', handleResize);
		drawStars();

		// Add hover event listeners for anime-style speed boost
		canvas.addEventListener('mouseenter', () => {
			targetSpeed = boostSpeed;
		});

		canvas.addEventListener('mouseleave', () => {
			targetSpeed = baseSpeed;
		});

		return () => {
			window.removeEventListener('resize', handleResize);
			if (animationFrame) {
				cancelAnimationFrame(animationFrame);
			}
		};
	});
</script>

<canvas bind:this={canvas}></canvas>

<style lang="scss">
	canvas {
		position: absolute;
		top: 0;
		left: 0;
		width: 100%;
		height: 100%;
	}
</style>
