<script lang="ts">
	import { onMount, onDestroy } from 'svelte';

	// Props
	export let gradientSize: number = 1.6; // Default to 15% of container size (0.1 = 10%, 1.0 = 100%)

	let canvas: HTMLCanvasElement;
	let ctx: CanvasRenderingContext2D;
	let mouseX = 0;
	let mouseY = 0;
	let lastMouseX = 0; // Last position where gradient was drawn
	let lastMouseY = 0;
	let isHovering = false;
	let gradientOpacity = 0;
	let resizeObserver: ResizeObserver | null = null;
	let animationId: number;

	// Cache frequently accessed values
	let cachedRect: DOMRect;
	let cachedBackgroundColor: string;
	let cachedDitherColor: string;
	let needsRedraw = true;

	// Function to get CSS custom property values (with caching)
	function updateCachedCSSVariables() {
		if (!canvas) return;
		const computedStyle = getComputedStyle(canvas);
		cachedBackgroundColor =
			computedStyle.getPropertyValue('--dither-background-color').trim() || '#e0f0f0';
		cachedDitherColor = computedStyle.getPropertyValue('--dither-color').trim() || '#4FD1C7';
	}

	function updateCachedRect() {
		if (!canvas) return;
		cachedRect = canvas.getBoundingClientRect();
	}

	function updateCanvasSize() {
		if (!canvas) return;

		updateCachedRect();
		const dpr = window.devicePixelRatio || 1;

		// Set canvas size to match display size with device pixel ratio
		canvas.width = cachedRect.width * dpr;
		canvas.height = cachedRect.height * dpr;

		// Scale the context to match device pixel ratio
		ctx.scale(dpr, dpr);

		// Update cached CSS variables
		updateCachedCSSVariables();

		// Set initial position to bottom right
		setInitialPosition();
		needsRedraw = true;
	}

	function setInitialPosition() {
		if (!canvas || !cachedRect) return;

		// Position gradient at bottom right with some margin
		const margin = Math.min(cachedRect.width, cachedRect.height) * 0.15; // 15% margin from edges
		lastMouseX = cachedRect.width - margin;
		lastMouseY = cachedRect.height - margin;

		// Also initialize current mouse position to match initial position
		// This ensures turbulence effects work correctly on initial load
		mouseX = lastMouseX;
		mouseY = lastMouseY;
	}

	onMount(() => {
		ctx = canvas.getContext('2d')!;

		// Initial size setup
		updateCanvasSize();

		// Set up ResizeObserver to handle size changes
		resizeObserver = new ResizeObserver(() => {
			updateCanvasSize();
		});

		resizeObserver.observe(canvas);

		// Start animation loop
		animate();
	});

	onDestroy(() => {
		if (resizeObserver) {
			resizeObserver.disconnect();
		}
		if (animationId) {
			cancelAnimationFrame(animationId);
		}
	});

	function handleMouseMove(event: MouseEvent) {
		if (!cachedRect) updateCachedRect();

		const newMouseX = event.clientX - cachedRect.left;
		const newMouseY = event.clientY - cachedRect.top;

		// Only trigger redraw if mouse moved significantly
		if (Math.abs(newMouseX - mouseX) > 1 || Math.abs(newMouseY - mouseY) > 1) {
			mouseX = newMouseX;
			mouseY = newMouseY;
			needsRedraw = true;

			// Update last position when actively hovering
			if (isHovering) {
				lastMouseX = mouseX;
				lastMouseY = mouseY;
			}
		}
	}

	function handleMouseEnter() {
		isHovering = true;
		needsRedraw = true;
	}

	function handleMouseLeave() {
		isHovering = false;
		needsRedraw = true;
	}

	function animate() {
		if (!ctx || !cachedRect) {
			animationId = requestAnimationFrame(animate);
			return;
		}

		// Set opacity to full immediately
		if (gradientOpacity < 1) {
			gradientOpacity = 1;
			needsRedraw = true;
		}

		// Only redraw if something changed
		if (needsRedraw && gradientOpacity > 0) {
			render();
			needsRedraw = false;
		}

		animationId = requestAnimationFrame(animate);
	}

	function render() {
		if (!ctx || !cachedRect) return;

		// Clear canvas with cached background color
		ctx.fillStyle = cachedBackgroundColor;
		ctx.fillRect(0, 0, cachedRect.width, cachedRect.height);

		// Use current position when hovering, last position when not
		const drawX = isHovering ? mouseX : lastMouseX;
		const drawY = isHovering ? mouseY : lastMouseY;
		drawDitheredCircle(drawX, drawY, gradientOpacity);
	}

	function drawDitheredCircle(centerX: number, centerY: number, opacity: number = 1) {
		if (!ctx || !cachedRect) return;

		// Scale parameters based on canvas size for adaptive behavior
		const baseSize = Math.min(cachedRect.width, cachedRect.height);
		const maxRadius = baseSize * gradientSize;
		const maxRadiusSquared = maxRadius * maxRadius;

		// Pre-calculate all expensive operations once
		const mouseNormX = (mouseX / cachedRect.width) * 2 - 1;
		const mouseNormY = (mouseY / cachedRect.height) * 2 - 1;

		const easedX = Math.sign(mouseNormX) * Math.pow(Math.abs(mouseNormX), 0.7);
		const easedY = Math.sign(mouseNormY) * Math.pow(Math.abs(mouseNormY), 0.7);

		const turbulenceStrength = 0.5;
		const stretchX = 1 + easedX * turbulenceStrength;
		const stretchY = 1 + easedY * turbulenceStrength;
		const shearX = easedY * 0.4;
		const shearY = easedX * 0.3;
		const rotationAngle = easedX * easedY * 0.3;

		// Pre-calculate rotation constants
		const cosRot = Math.cos(rotationAngle);
		const sinRot = Math.sin(rotationAngle);

		// Pre-calculate wave constants
		const waveFreq1 = 0.006;
		const waveFreq2 = 0.015;
		const waveAmplitude = maxRadius * 0.25;
		const absEasedX = Math.abs(easedX);
		const absEasedY = Math.abs(easedY);
		const piEasedX = absEasedX * Math.PI;
		const piEasedY = absEasedY * Math.PI;
		const waveAmp2 = waveAmplitude * 0.3;

		const dotSize = Math.max(1.5, baseSize * 0.001);
		const spacing = Math.max(4, baseSize * 0.001);
		const halfDotSize = dotSize * 0.5;
		const spacingInv = 1.0 / spacing; // Pre-calculate for division optimization
		const threshold25 = 0.25;

		ctx.fillStyle = cachedDitherColor;
		ctx.globalAlpha = opacity;

		const searchRadius = maxRadius * 1.5;
		const searchRadiusSquared = searchRadius * searchRadius;
		const minX = centerX - searchRadius;
		const maxX = centerX + searchRadius;
		const minY = centerY - searchRadius;
		const maxY = centerY + searchRadius;

		// Use batched drawing for better performance
		const rects: Array<{ x: number; y: number }> = [];

		for (let y = minY; y <= maxY; y += spacing) {
			const relY = y - centerY;
			const relYSquared = relY * relY;

			for (let x = minX; x <= maxX; x += spacing) {
				const relX = x - centerX;
				const relXSquared = relX * relX;

				// Early distance check before expensive transformations
				const basicDistanceSquared = relXSquared + relYSquared;
				if (basicDistanceSquared > searchRadiusSquared) continue;

				// Apply transformations (optimized)
				const rotatedX = relX * cosRot - relY * sinRot;
				const rotatedY = relX * sinRot + relY * cosRot;

				const transformedX = rotatedX * stretchX + rotatedY * shearX;
				const transformedY = rotatedY * stretchY + rotatedX * shearY;

				// Optimize wave calculations
				const distance = Math.sqrt(basicDistanceSquared);
				const angle = Math.atan2(relY, relX);
				const wavePhaseX = piEasedX + angle * 0.5;
				const wavePhaseY = piEasedY + distance * 0.01;

				const wave1X = Math.sin(transformedY * waveFreq1 + wavePhaseX) * waveAmplitude * absEasedX;
				const wave1Y = Math.cos(transformedX * waveFreq1 + wavePhaseY) * waveAmplitude * absEasedY;

				const wave2X = Math.sin(transformedY * waveFreq2 - wavePhaseX * 0.7) * waveAmp2 * absEasedY;
				const wave2Y = Math.cos(transformedX * waveFreq2 - wavePhaseY * 0.7) * waveAmp2 * absEasedX;

				const finalX = transformedX + wave1X + wave2X;
				const finalY = transformedY + wave1Y + wave2Y;
				const finalDistanceSquared = finalX * finalX + finalY * finalY;

				if (finalDistanceSquared < maxRadiusSquared) {
					const finalDistance = Math.sqrt(finalDistanceSquared);
					const intensity = 1 - finalDistance / maxRadius;

					// Optimize grid calculations
					const gridX = Math.floor(x * spacingInv) & 1;
					const gridY = Math.floor(y * spacingInv) & 1;
					const threshold = (gridX + gridY * 2) * threshold25;

					if (intensity > threshold) {
						rects.push({ x: x - halfDotSize, y: y - halfDotSize });
					}
				}
			}
		}

		// Batch draw all rectangles
		for (let i = 0; i < rects.length; i++) {
			const rect = rects[i];
			ctx.fillRect(rect.x, rect.y, dotSize, dotSize);
		}

		ctx.globalAlpha = 1;
	}
</script>

<canvas
	bind:this={canvas}
	on:mousemove={handleMouseMove}
	on:mouseenter={handleMouseEnter}
	on:mouseleave={handleMouseLeave}
></canvas>

<style>
	canvas {
		display: block;
		z-index: 0;
		position: absolute;
		top: 0;
		left: 0;
		width: 100%;
		height: 100%;

		/* CSS custom properties for dithering */
		--dither-color: var(--clr-scale-pop-50);
		--dither-background-color: var(--clr-theme-pop-soft);
	}
</style>
