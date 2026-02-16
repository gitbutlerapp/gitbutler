<script lang="ts" module>
	export interface Props {
		picture?: string;
		alt?: string;
		acceptedFileTypes?: string[];
		onFileSelect?: (file: File) => void;
		onInvalidFileType?: () => void;
		class?: string;
		size?: string;
	}
</script>

<script lang="ts">
	import SkeletonBone from "$components/SkeletonBone.svelte";
	import { useImageLoading } from "$lib/utils/imageLoading.svelte";

	let {
		picture = $bindable(),
		alt = "",
		acceptedFileTypes = ["image/jpeg", "image/png"],
		onFileSelect,
		onInvalidFileType,
		class: className,
		size = "6.25rem",
	}: Props = $props();

	let previewUrl = $derived(picture);
	const imageLoadingState = useImageLoading();

	function handleFileChange(e: Event) {
		const target = e.target as HTMLInputElement;
		const file = target.files?.[0];

		if (file && acceptedFileTypes.includes(file.type)) {
			picture = URL.createObjectURL(file);
			onFileSelect?.(file);
		} else {
			onInvalidFileType?.();
		}
	}
</script>

<label
	class="profile-pic-wrapper {className || ''}"
	for="profile-picture-upload"
	style:width={size}
>
	<input
		onchange={handleFileChange}
		type="file"
		id="profile-picture-upload"
		name="picture"
		accept={acceptedFileTypes.join(",")}
		class="hidden-input"
	/>

	{#if !previewUrl || !imageLoadingState.imageLoaded || imageLoadingState.hasError}
		<div class="profile-pic-skeleton">
			<SkeletonBone width="100%" height="100%" radius="var(--radius-m)" />
		</div>
	{/if}

	{#if previewUrl && !imageLoadingState.hasError}
		<img
			bind:this={imageLoadingState.imgElement}
			class="profile-pic"
			class:loaded={imageLoadingState.imageLoaded}
			src={previewUrl}
			{alt}
			referrerpolicy="no-referrer"
			onload={imageLoadingState.handleImageLoad}
			onerror={imageLoadingState.handleImageError}
		/>
	{/if}

	<span class="profile-pic__edit-label text-11 text-semibold">Edit</span>
</label>

<style lang="postcss">
	.profile-pic-wrapper {
		display: flex;
		position: relative;
		flex-shrink: 0;
		aspect-ratio: 1 / 1;
		height: max-content;
		overflow: hidden;
		border-radius: var(--radius-ml);
		cursor: pointer;

		&:hover,
		&:focus-within {
			& .profile-pic__edit-label {
				transform: translateY(0);
				opacity: 1;
			}
		}
	}

	.profile-pic-skeleton {
		z-index: var(--z-ground);
		position: absolute;
		width: 100%;
		height: 100%;
	}

	.hidden-input {
		z-index: var(--z-ground);
		position: absolute;
		width: 100%;
		height: 100%;
		cursor: pointer;
		opacity: 0;
	}

	.profile-pic {
		width: 100%;
		height: 100%;
		object-fit: cover;
		background-color: var(--clr-core-pop-70);
		opacity: 0;
		transition: opacity 0.2s ease-in;

		&.loaded {
			opacity: 1;
		}
	}

	.profile-pic__edit-label {
		position: absolute;
		bottom: 8px;
		left: 8px;
		padding: 4px 6px;
		transform: translateY(2px);
		border-radius: var(--radius-m);
		outline: 1px solid color-mix(in srgb, var(--clr-core-gray-100) 40%, transparent);
		background-color: var(--clr-core-gray-20);
		color: var(--clr-core-gray-100);
		opacity: 0;
		transition:
			opacity var(--transition-fast),
			transform var(--transition-medium);
	}
</style>
