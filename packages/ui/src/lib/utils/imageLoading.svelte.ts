/**
 * Hook for tracking image loading state with cache detection
 * @returns Object with imageLoaded state, imgElement binding, and handleImageLoad callback
 */
export function useImageLoading() {
	let imageLoaded = $state(false);
	let imgElement: HTMLImageElement | undefined = $state();

	// Check if image is already loaded from cache or when element updates
	$effect(() => {
		if (imgElement) {
			// If image is already complete (cached), set loaded immediately
			if (imgElement.complete && imgElement.naturalWidth > 0) {
				imageLoaded = true;
			} else {
				// Reset for new images
				imageLoaded = false;
			}
		}
	});

	function handleImageLoad() {
		imageLoaded = true;
	}

	return {
		get imageLoaded() {
			return imageLoaded;
		},
		set imgElement(value: HTMLImageElement | undefined) {
			imgElement = value;
		},
		get imgElement() {
			return imgElement;
		},
		handleImageLoad
	};
}
