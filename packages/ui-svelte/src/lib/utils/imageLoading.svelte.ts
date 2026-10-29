/**
 * Hook for tracking image loading state with cache detection
 * @returns Object with imageLoaded state, imgElement binding, and handleImageLoad callback
 */
export function useImageLoading() {
	let imageLoaded = $state(false);
	let hasError = $state(false);
	let imgElement: HTMLImageElement | undefined = $state();

	// Check if image is already loaded from cache or when element updates
	$effect(() => {
		if (imgElement) {
			// If image is already complete (cached), set loaded immediately
			if (imgElement.complete && imgElement.naturalWidth > 0) {
				imageLoaded = true;
				hasError = false;
			} else {
				// Reset for new images
				imageLoaded = false;
				hasError = false;
			}
		}
	});

	function handleImageLoad() {
		imageLoaded = true;
		hasError = false;
	}

	function handleImageError() {
		// Silently handle image load errors and use fallback
		imageLoaded = false;
		hasError = true;
	}

	return {
		get imageLoaded() {
			return imageLoaded;
		},
		get hasError() {
			return hasError;
		},
		set imgElement(value: HTMLImageElement | undefined) {
			imgElement = value;
		},
		get imgElement() {
			return imgElement;
		},
		handleImageLoad,
		handleImageError,
	};
}
