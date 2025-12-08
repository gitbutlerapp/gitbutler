<script lang="ts">
	import Badge from '$components/Badge.svelte';
	import RangeInput from '$components/RangeInput.svelte';
	import SkeletonBone from '$components/SkeletonBone.svelte';
	import Segment from '$components/segmentControl/Segment.svelte';
	import SegmentControl from '$components/segmentControl/SegmentControl.svelte';

	type Props = {
		beforeImageUrl?: string | null;
		afterImageUrl?: string | null;
		fileName?: string;
		isLoading?: boolean;
	};

	const {
		beforeImageUrl = null,
		afterImageUrl = null,
		fileName = 'image',
		isLoading = false
	}: Props = $props();

	type ViewMode = '2-up' | 'swipe' | 'onion-skin';
	let viewMode = $state<ViewMode>('2-up');
	let swipePosition = $state(50);
	let onionOpacity = $state(50);
	let isDragging = $state(false);
	let comparisonWrapperRef: HTMLDivElement | undefined = $state();

	type ImageMetadata = {
		width: number;
		height: number;
		size?: number;
	};

	let beforeImageMetadata = $state<ImageMetadata | null>(null);
	let afterImageMetadata = $state<ImageMetadata | null>(null);

	function formatFileSize(bytes: number): string {
		if (bytes < 1024) return `${bytes} B`;
		if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
		return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
	}

	function formatSizeDifference(beforeSize: number, afterSize: number): string {
		const diff = afterSize - beforeSize;
		const absDiff = Math.abs(diff);

		if (diff > 0) {
			return `+${formatFileSize(absDiff)}`;
		} else if (diff < 0) {
			return `-${formatFileSize(absDiff)}`;
		}
		return 'Same size';
	}

	async function loadImageMetadata(url: string): Promise<ImageMetadata> {
		return new Promise((resolve, reject) => {
			const img = new Image();
			img.onload = async () => {
				const metadata: ImageMetadata = {
					width: img.naturalWidth,
					height: img.naturalHeight
				};

				// Try to fetch file size
				try {
					const response = await fetch(url);
					const blob = await response.blob();
					metadata.size = blob.size;
				} catch {
					// Size not available, continue without it
				}
				resolve(metadata);
			};
			img.onerror = () => reject(new Error('Failed to load image'));
			img.src = url;
		});
	}

	$effect(() => {
		if (beforeImageUrl) {
			loadImageMetadata(beforeImageUrl)
				.then((metadata) => (beforeImageMetadata = metadata))
				.catch((error) => {
					console.error('Failed to load before image metadata:', error);
					beforeImageMetadata = null;
				});
		}

		if (afterImageUrl) {
			loadImageMetadata(afterImageUrl)
				.then((metadata) => (afterImageMetadata = metadata))
				.catch((error) => {
					console.error('Failed to load after image metadata:', error);
					afterImageMetadata = null;
				});
		}
	});

	function updateSwipePosition(clientX: number) {
		if (!comparisonWrapperRef) return;

		const rect = comparisonWrapperRef.getBoundingClientRect();
		const x = clientX - rect.left;
		swipePosition = Math.max(0, Math.min(100, (x / rect.width) * 100));
	}

	function handleDragStart(e: MouseEvent | TouchEvent) {
		isDragging = true;
		const clientX = e instanceof MouseEvent ? e.clientX : e.touches[0].clientX;
		updateSwipePosition(clientX);
	}

	function handleDragMove(e: MouseEvent | TouchEvent) {
		if (!isDragging) return;
		if (e instanceof TouchEvent) e.preventDefault();
		const clientX = e instanceof MouseEvent ? e.clientX : e.touches[0].clientX;
		updateSwipePosition(clientX);
	}

	function handleDragEnd() {
		isDragging = false;
	}

	function handleKeyDown(e: KeyboardEvent) {
		const step = e.shiftKey ? 10 : 1; // Larger steps with Shift key

		switch (e.key) {
			case 'ArrowLeft':
			case 'ArrowDown':
				e.preventDefault();
				swipePosition = Math.max(0, swipePosition - step);
				break;
			case 'ArrowRight':
			case 'ArrowUp':
				e.preventDefault();
				swipePosition = Math.min(100, swipePosition + step);
				break;
			case 'Home':
				e.preventDefault();
				swipePosition = 0;
				break;
			case 'End':
				e.preventDefault();
				swipePosition = 100;
				break;
		}
	}

	$effect(() => {
		if (viewMode !== 'swipe') return;

		document.addEventListener('mousemove', handleDragMove);
		document.addEventListener('mouseup', handleDragEnd);
		document.addEventListener('touchmove', handleDragMove, { passive: false });
		document.addEventListener('touchend', handleDragEnd);

		return () => {
			document.removeEventListener('mousemove', handleDragMove);
			document.removeEventListener('mouseup', handleDragEnd);
			document.removeEventListener('touchmove', handleDragMove);
			document.removeEventListener('touchend', handleDragEnd);
		};
	});
</script>

{#snippet imageDimensions(metadata: ImageMetadata | null)}
	{#if metadata}
		<span>
			{metadata.width}×{metadata.height}
			{#if metadata.size}
				· {formatFileSize(metadata.size)}
			{/if}
		</span>
	{/if}
{/snippet}

{#snippet sizeDifference()}
	{#if beforeImageMetadata?.size && afterImageMetadata?.size}
		·
		<span
			class:positive={afterImageMetadata.size < beforeImageMetadata.size}
			class:negative={afterImageMetadata.size > beforeImageMetadata.size}
		>
			{formatSizeDifference(beforeImageMetadata.size, afterImageMetadata.size)}
			{#if afterImageMetadata.size < beforeImageMetadata.size}
				<span aria-label="decreased">↘</span>
			{:else if afterImageMetadata.size > beforeImageMetadata.size}
				<span aria-label="increased">↗</span>
			{/if}
		</span>
	{/if}
{/snippet}

{#snippet comparisonFooter()}
	<div class="text-12 image-footer">
		{@render imageDimensions(beforeImageMetadata)}
		→
		{@render imageDimensions(afterImageMetadata)}
		{@render sizeDifference()}
	</div>
{/snippet}

{#snippet imagePanel(props: {
	url: string;
	label: string;
	isBefore?: boolean;
	metadata: ImageMetadata | null;
})}
	<div class="image-panel {props.isBefore ? 'before' : 'after'}">
		<div class="image-wrapper checkered-bg">
			<img src={props.url} alt="{fileName} ({props.label})" />
		</div>

		<div class="text-12 image-footer">
			{#if props.isBefore}
				<Badge style="error" class="label">{props.label}</Badge>
			{:else}
				<Badge style="success" class="label">{props.label}</Badge>
			{/if}
			·
			{@render imageDimensions(props.metadata)}
			{#if !props.isBefore}
				{@render sizeDifference()}
			{/if}
		</div>
	</div>
{/snippet}

{#snippet swipe(props: { controlValue: number; onValueChange: (value: number) => void })}
	<div class="comparison-container">
		<div
			class="comparison-wrapper is-swipe checkered-bg"
			class:is-dragging={isDragging}
			bind:this={comparisonWrapperRef}
			onmousedown={handleDragStart}
			ontouchstart={handleDragStart}
			onkeydown={handleKeyDown}
			role="slider"
			tabindex="0"
			aria-label="Image comparison slider"
			aria-valuenow={props.controlValue}
			aria-valuemin={0}
			aria-valuemax={100}
		>
			<div class="comparison-image comparison-after">
				<img src={afterImageUrl!} alt="{fileName} (After)" />
			</div>
			<div
				class="comparison-image comparison-before"
				style="clip-path: inset(0 {100 - props.controlValue}% 0 0);"
			>
				<img src={beforeImageUrl!} alt="{fileName} (Before)" />
			</div>
			<div class="swipe-handle" style="left: {props.controlValue}%">
				<div class="swipe-divider"></div>
				<div class="swipe-handle-grip text-14">↔</div>
			</div>
		</div>
		{@render comparisonFooter()}
	</div>
{/snippet}

{#snippet onionSkin(props: { controlValue: number; onValueChange: (value: number) => void })}
	<div class="comparison-container">
		<div class="comparison-wrapper checkered-bg" bind:this={comparisonWrapperRef}>
			<div class="comparison-image comparison-after">
				<img src={afterImageUrl!} alt="{fileName} (After)" />
			</div>
			<div class="comparison-image comparison-before" style="opacity: {props.controlValue / 100};">
				<img src={beforeImageUrl!} alt="{fileName} (Before)" />
			</div>
		</div>
		<div class="comparison-controls">
			<Badge style="error" kind="soft">Before</Badge>
			<RangeInput min={0} max={100} value={props.controlValue} oninput={props.onValueChange} wide />
			<Badge style="success" kind="soft">After</Badge>
		</div>
		{@render comparisonFooter()}
	</div>
{/snippet}

<div class="imagediff-container">
	{#if beforeImageUrl && afterImageUrl && !isLoading}
		<div class="view-mode-controls">
			<SegmentControl size="small" defaultIndex={0} onselect={(id) => (viewMode = id as ViewMode)}>
				<Segment id="2-up">2-up</Segment>
				<Segment id="swipe">Swipe</Segment>
				<Segment id="onion-skin">Onion Skin</Segment>
			</SegmentControl>
		</div>
	{/if}

	<div class="image-comparison" class:is-swipe={viewMode === 'swipe'}>
		{#if isLoading}
			<div class="image-panel skeleton-panel">
				<SkeletonBone height="12.5rem" />
				<div class="skeleton-footer">
					<SkeletonBone width="3.75rem" height="1.25rem" />
					<SkeletonBone width="6.25rem" height="0.75rem" />
				</div>
			</div>
			<div class="image-panel skeleton-panel">
				<SkeletonBone height="12.5rem" />
				<div class="skeleton-footer">
					<SkeletonBone width="3.75rem" height="1.25rem" />
					<SkeletonBone width="6.25rem" height="0.75rem" />
				</div>
			</div>
		{:else if beforeImageUrl || afterImageUrl}
			{#if viewMode === '2-up'}
				{#if beforeImageUrl}
					{@render imagePanel({
						url: beforeImageUrl,
						label: afterImageUrl ? 'Before' : 'Removed',
						isBefore: true,
						metadata: beforeImageMetadata
					})}
				{/if}

				{#if afterImageUrl}
					{@render imagePanel({
						url: afterImageUrl,
						label: beforeImageUrl ? 'After' : 'Added',
						metadata: afterImageMetadata
					})}
				{/if}
			{:else if viewMode === 'swipe' && beforeImageUrl && afterImageUrl}
				{@render swipe({
					controlValue: swipePosition,
					onValueChange: (value) => (swipePosition = value)
				})}
			{:else if viewMode === 'onion-skin' && beforeImageUrl && afterImageUrl}
				{@render onionSkin({
					controlValue: onionOpacity,
					onValueChange: (value) => (onionOpacity = value)
				})}
			{/if}
		{/if}
	</div>
</div>

<style lang="postcss">
	.imagediff-container {
		container-type: inline-size;
		display: flex;
		flex-direction: column;
		gap: 14px;
	}

	.view-mode-controls {
		display: flex;
	}

	.image-comparison {
		display: flex;
		gap: 14px;
	}

	@container (width < 600px) {
		.image-comparison {
			flex-direction: column;
		}
	}

	.image-panel {
		display: flex;
		flex: 1;
		flex-direction: column;
	}

	.image-footer {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		margin-top: 10px;
		gap: 6px;
		color: var(--clr-text-2);

		& .negative {
			color: var(--clr-scale-err-40);
		}
		& .positive {
			color: var(--clr-scale-succ-40);
		}
	}

	/* Shared checkered background pattern */
	.checkered-bg {
		background-image:
			linear-gradient(45deg, var(--clr-bg-3) 25%, transparent 25%),
			linear-gradient(-45deg, var(--clr-bg-3) 25%, transparent 25%),
			linear-gradient(45deg, transparent 75%, var(--clr-bg-3) 75%),
			linear-gradient(-45deg, transparent 75%, var(--clr-bg-3) 75%);
		background-position:
			0 0,
			0 12px,
			12px -12px,
			-12px 0px;
		background-size: 24px 24px;
	}

	.image-wrapper {
		display: flex;
		align-items: center;
		justify-content: center;
		min-height: 200px;
		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
	}

	.image-wrapper img {
		display: block;
		max-width: 100%;
		max-height: 600px;
		object-fit: contain;
	}

	.skeleton-panel {
		display: flex;
		flex-direction: column;
		gap: 10px;
	}

	.skeleton-footer {
		display: flex;
		align-items: center;
		gap: 8px;
	}

	/* Comparison modes (swipe & onion-skin) shared styles */
	.comparison-container {
		display: flex;
		flex: 1;
		flex-direction: column;
	}

	.comparison-wrapper {
		display: flex;
		position: relative;
		align-items: center;
		justify-content: center;
		min-height: 400px;
		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		border-bottom: none;
		border-radius: var(--radius-m) var(--radius-m) 0 0;
		cursor: col-resize;

		&.is-dragging {
			user-select: none;
		}

		&.is-swipe {
			border-bottom: 1px solid var(--clr-border-2);
			border-radius: var(--radius-m) var(--radius-m);
			cursor: ew-resize;
		}
	}

	.comparison-image {
		display: flex;
		position: absolute;
		top: 0;
		right: 0;
		bottom: 0;
		left: 0;
		align-items: center;
		justify-content: center;
		pointer-events: none;
	}

	.comparison-image img {
		display: block;
		max-width: 100%;
		max-height: 100%;
		object-fit: contain;
	}

	.comparison-before img {
		border-radius: var(--radius-s);
	}

	.swipe-handle {
		display: flex;
		z-index: var(--z-index-lifted);
		position: absolute;
		top: 0;
		bottom: 0;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		transform: translateX(-50%);
		pointer-events: none;
	}

	.swipe-divider {
		position: absolute;
		top: 0;
		bottom: 0;
		width: 2px;
		background-color: var(--clr-core-ntrl-0);
	}

	.swipe-handle-grip {
		display: flex;
		flex-shrink: 0;
		align-items: center;
		justify-content: center;
		padding: 4px 6px;
		border-radius: 20px;
		background-color: var(--clr-core-ntrl-0);
		color: var(--clr-core-ntrl-100);
		cursor: col-resize;
		pointer-events: all;
	}

	.comparison-controls {
		display: flex;
		align-items: center;
		padding: 12px;
		gap: 10px;
		border: 1px solid var(--clr-border-2);
		border-radius: 0 0 var(--radius-m) var(--radius-m);
	}
</style>
