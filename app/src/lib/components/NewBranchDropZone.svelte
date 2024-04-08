<script lang="ts">
	// import images
	import bottomSheetSvg from '$lib/assets/new-branch/bottom-sheet.svg?raw';
	import handSvg from '$lib/assets/new-branch/hand.svg?raw';
	import middleSheetSvg from '$lib/assets/new-branch/middle-sheet.svg?raw';
	import topSheetSvg from '$lib/assets/new-branch/top-sheet.svg?raw';
	// import components
	import Button from '$lib/components/Button.svelte';
	import { DraggableFile, DraggableHunk } from '$lib/dragging/draggables';
	import { dropzone } from '$lib/dragging/dropzone';
	import { getContext } from '$lib/utils/context';
	import { BranchController } from '$lib/vbranches/branchController';
	import { filesToOwnership } from '$lib/vbranches/ownership';

	const branchController = getContext(BranchController);

	function accepts(data: any) {
		if (data instanceof DraggableFile) return !data.files.some((f) => f.locked);
		if (data instanceof DraggableHunk) return !data.hunk.locked;
		return false;
	}

	function onDrop(data: DraggableFile | DraggableHunk) {
		if (data instanceof DraggableHunk) {
			const ownership = `${data.hunk.filePath}:${data.hunk.id}`;
			branchController.createBranch({ ownership });
		} else if (data instanceof DraggableFile) {
			const ownership = filesToOwnership(data.files);
			branchController.createBranch({ ownership });
		}
	}
</script>

<div
	class="canvas-dropzone"
	use:dropzone={{
		active: 'new-dz-active',
		hover: 'new-dz-hover',
		onDrop,
		accepts
	}}
>
	<div id="new-branch-dz" class="new-virtual-branch">
		<div class="new-virtual-branch__content">
			<div class="stimg">
				<div class="stimg__hand">
					{@html handSvg}
				</div>
				<div class="stimg__top-sheet">
					{@html topSheetSvg}
				</div>
				<div class="stimg__middle-sheet">
					{@html middleSheetSvg}
				</div>
				<div class="stimg__bottom-sheet">
					{@html bottomSheetSvg}
				</div>

				<div class="stimg__branch">
					<div class="stimg__branch-plus" />
				</div>
			</div>

			<span class="text-base-body-13 new-branch-caption"
				>Drag and drop files<br />to create a new branch</span
			>
		</div>
		<div class="new-branch-button">
			<Button
				style="ghost"
				kind="solid"
				icon="plus-small"
				on:mousedown={() => branchController.createBranch({})}>New branch</Button
			>
		</div>
	</div>
</div>

<style lang="postcss">
	.canvas-dropzone {
		height: 100%;
		user-select: none;
		display: flex;
		padding: var(--size-12);
	}

	.new-virtual-branch {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		width: 22rem;
		border-radius: var(--radius-m);
		border: 1px dashed var(--clr-border-main);
		background-color: transparent;
		padding: var(--size-20);
		gap: var(--size-8);

		outline-color: transparent;
		outline-style: dashed;
		outline-width: 1px;
		outline-offset: -1px;

		transition:
			outline-offset var(--transition-medium),
			outline-color var(--transition-medium),
			border var(--transition-medium),
			background-color var(--transition-medium);
	}

	.new-virtual-branch__content {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: var(--size-12);
		transition: transform var(--transition-medium);
		padding: var(--size-20) var(--size-24) var(--size-16) var(--size-24);
	}

	/* ILLUSTRATION */
	.stimg {
		position: relative;
		width: 120px;
		height: 130px;
		opacity: 0.8;

		& div {
			position: absolute;
			transition:
				transform var(--transition-medium),
				opacity var(--transition-medium),
				filter var(--transition-medium);
		}
	}

	.stimg__hand {
		z-index: 5;
		top: 85px;
		left: 25px;
	}

	.stimg__top-sheet {
		z-index: 4;
		top: 56px;
		left: 8px;
		width: 44px;
		height: 53px;
	}

	.stimg__middle-sheet {
		z-index: 3;
		top: 42px;
		left: 23px;
		width: 44px;
		height: 40px;
	}

	.stimg__bottom-sheet {
		z-index: 2;
		top: 66px;
		left: 44px;
		width: 44px;
		height: 41px;
	}

	.stimg__branch {
		z-index: 1;
		top: 0;
		left: 35px;
		width: 77px;
		height: 83px;
		background-color: oklch(from var(--clr-scale-ntrl-60) l c h / 0.2);
		border-radius: 12px;
	}

	.stimg__branch-plus {
		position: absolute;
		top: 16px;
		left: 50%;
		transform: translateX(-50%);
		width: 34px;
		height: 34px;
		opacity: 0.3;

		&::before,
		&::after {
			content: '';
			position: absolute;
			top: 50%;
			left: 50%;
			transform: translate(-50%, -50%);
			width: 100%;
			height: 2px;
			background-color: var(--clr-scale-ntrl-20);
		}

		&::after {
			transform: translate(-50%, -50%) rotate(90deg);
		}
	}

	/* OTHER */

	.new-branch-caption {
		color: var(--clr-scale-ntrl-0);
		text-align: center;
		opacity: 0.25;
		transition: opacity var(--transition-medium);
	}

	.new-branch-button {
		transition: opacity var(--transition-medium);
	}

	/* DRAGZONE MODIEFIERS */
	.canvas-dropzone {
		&:global(.new-dz-active) {
			& .new-virtual-branch {
				background-color: oklch(from var(--clr-scale-pop-70) l c h / 0.1);
				border: 1px dashed oklch(from var(--clr-scale-pop-40) l c h / 0.8);
				color: var(--clr-scale-pop-50);
			}

			& .new-virtual-branch__content {
				transform: translateY(0.5rem);
			}

			& .new-branch-button {
				opacity: 0;
			}

			& .stimg {
				opacity: 1;
			}

			& .stimg__hand {
				transform: translate(4px, 3px) rotate(5deg) scale(0.9);
			}

			& .stimg__top-sheet {
				transform: translate(3px, 4px);
			}

			& .stimg__middle-sheet {
				transform: translate(-2px, 7px);
			}

			& .stimg__bottom-sheet {
				transform: translate(-5px, 0);
			}

			& .stimg__branch-plus {
				opacity: 0.4;

				&::before,
				&::after {
					width: 110%;
				}
			}
		}
	}
</style>
