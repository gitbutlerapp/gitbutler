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

		// Position spot on the left side of canvas
		lastMouseX = cachedRect.width * 0.5;
		lastMouseY = cachedRect.height / 2.3;

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
		drawDitheredCanvasWithClearArea(drawX, drawY, gradientOpacity);
	}

	function drawDitheredCanvasWithClearArea(centerX: number, centerY: number, opacity: number = 1) {
		if (!ctx || !cachedRect) return;

		// Scale parameters based on canvas size for adaptive behavior - elliptical
		const ellipseRadiusX = cachedRect.width * gradientSize * 1; // Width-based radius, bigger than canvas
		const ellipseRadiusY = cachedRect.height * gradientSize * 0.8; // Height-based radius, bigger than canvas
		const ellipseRadiusXSquared = ellipseRadiusX * ellipseRadiusX;
		const ellipseRadiusYSquared = ellipseRadiusY * ellipseRadiusY;

		// Pre-calculate all expensive operations once
		const mouseNormX = (centerX / cachedRect.width) * 2 - 1;
		const mouseNormY = (centerY / cachedRect.height) * 2 - 1;

		const easedX = Math.sign(mouseNormX) * Math.pow(Math.abs(mouseNormX), 0.7);
		const easedY = Math.sign(mouseNormY) * Math.pow(Math.abs(mouseNormY), 0.7);

		const turbulenceStrength = 0.1; // Reduced from 0.5 to 0.1
		const stretchX = 1 + easedX * turbulenceStrength;
		const stretchY = 1 + easedY * turbulenceStrength;
		const shearX = easedY * 0.1; // Reduced from 0.4 to 0.1
		const shearY = easedX * 0.1; // Reduced from 0.3 to 0.1
		const rotationAngle = easedX * easedY * 0.1; // Reduced from 0.3 to 0.1

		// Pre-calculate rotation constants
		const cosRot = Math.cos(rotationAngle);
		const sinRot = Math.sin(rotationAngle);

		// Pre-calculate wave constants
		const waveFreq1 = 0.006;
		const waveFreq2 = 0.015;
		const waveAmplitude = Math.max(ellipseRadiusX, ellipseRadiusY) * 0.05; // Reduced from 0.25 to 0.05
		const absEasedX = Math.abs(easedX);
		const absEasedY = Math.abs(easedY);
		const piEasedX = absEasedX * Math.PI;
		const piEasedY = absEasedY * Math.PI;
		const waveAmp2 = waveAmplitude * 0.3;

		const baseSize = Math.min(cachedRect.width, cachedRect.height);
		const dotSize = Math.max(1.3, baseSize * 0.01);
		const spacing = Math.max(4, baseSize * 0.01); // Increased spacing for fewer dots
		const halfDotSize = dotSize * 0.5;
		const spacingInv = 1.0 / spacing; // Pre-calculate for division optimization
		const threshold25 = 0.25;

		ctx.fillStyle = cachedDitherColor;
		ctx.globalAlpha = opacity;

		// Draw dithered pattern across the entire canvas, but skip the clear area
		const canvasWidth = cachedRect.width;
		const canvasHeight = cachedRect.height;

		// Use batched drawing for better performance
		const rects: Array<{ x: number; y: number }> = [];

		for (let y = 0; y <= canvasHeight; y += spacing) {
			for (let x = 0; x <= canvasWidth; x += spacing) {
				// Check if this point is within the clear area
				const relX = x - centerX;
				const relY = y - centerY;
				const relXSquared = relX * relX;
				const relYSquared = relY * relY;
				const basicDistanceSquared = relXSquared + relYSquared;

				// Check if this point is within the elliptical gradient area
				let gradientIntensity = 0;
				// Quick ellipse check first
				const ellipseCheck =
					relXSquared / ellipseRadiusXSquared + relYSquared / ellipseRadiusYSquared;
				if (ellipseCheck <= 2.25) {
					// Expand check area for transformations
					// Apply transformations to determine the gradient intensity
					const rotatedX = relX * cosRot - relY * sinRot;
					const rotatedY = relX * sinRot + relY * cosRot;

					const transformedX = rotatedX * stretchX + rotatedY * shearX;
					const transformedY = rotatedY * stretchY + rotatedX * shearY;

					// Apply wave distortions
					const distance = Math.sqrt(basicDistanceSquared);
					const angle = Math.atan2(relY, relX);
					const wavePhaseX = piEasedX + angle * 0.5;
					const wavePhaseY = piEasedY + distance * 0.01;

					const wave1X =
						Math.sin(transformedY * waveFreq1 + wavePhaseX) * waveAmplitude * absEasedX;
					const wave1Y =
						Math.cos(transformedX * waveFreq1 + wavePhaseY) * waveAmplitude * absEasedY;

					const wave2X =
						Math.sin(transformedY * waveFreq2 - wavePhaseX * 0.7) * waveAmp2 * absEasedY;
					const wave2Y =
						Math.cos(transformedX * waveFreq2 - wavePhaseY * 0.7) * waveAmp2 * absEasedX;

					const finalX = transformedX + wave1X + wave2X;
					const finalY = transformedY + wave1Y + wave2Y;

					// Check if final transformed point is within the ellipse
					const finalEllipseCheck =
						(finalX * finalX) / ellipseRadiusXSquared + (finalY * finalY) / ellipseRadiusYSquared;

					if (finalEllipseCheck < 1.0) {
						gradientIntensity = 1 - Math.sqrt(finalEllipseCheck);
					}
				}

				// Apply dithering pattern with gradient effect
				const gridX = Math.floor(x * spacingInv) & 1;
				const gridY = Math.floor(y * spacingInv) & 1;
				const threshold = (gridX + gridY * 3) * threshold25;

				// Calculate final intensity: base background intensity reduced by gradient
				let finalIntensity = 0.5; // Reduced base intensity for fewer dots

				if (gradientIntensity > 0) {
					// Create a more contrasted gradient with completely clear center
					// Use a power curve to make the transition more dramatic
					const contrastCurve = Math.pow(gradientIntensity, 0.3); // Sharper falloff
					finalIntensity = 0.5 * (1 - contrastCurve);

					// Ensure complete clearing in the center area (when gradient intensity is high)
					if (gradientIntensity > 0.7) {
						finalIntensity = 0;
					}
				}

				if (finalIntensity > threshold) {
					rects.push({ x: x - halfDotSize, y: y - halfDotSize });
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
