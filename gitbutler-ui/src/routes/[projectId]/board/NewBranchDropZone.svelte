<script lang="ts">
	import {
		isDraggableHunk,
		isDraggableFile,
		type DraggableFile,
		type DraggableHunk
	} from '$lib/draggables';
	import { dropzone } from '$lib/utils/draggable';
	import type { BranchController } from '$lib/vbranches/branchController';

	import Button from '$lib/components/Button.svelte';

	import HandImg from './empty-state-img/HandImg.svelte';
	import TopSheetImg from './empty-state-img/TopSheetImg.svelte';
	import MiddleSheetImg from './empty-state-img/MiddleSheetImg.svelte';
	import BottomSheetImg from './empty-state-img/BottomSheetImg.svelte';

	export let branchController: BranchController;

	function accepts(data: any) {
		return isDraggableFile(data) || isDraggableHunk(data);
	}

	function onDrop(data: DraggableFile | DraggableHunk) {
		if (isDraggableHunk(data)) {
			const ownership = `${data.hunk.filePath}:${data.hunk.id}`;
			branchController.createBranch({ ownership });
		} else if (isDraggableFile(data)) {
			const ownership = `${data.file.path}:${data.file.hunks.map(({ id }) => id).join(',')}`;
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
					<HandImg />
				</div>
				<div class="stimg__top-sheet">
					<TopSheetImg />
				</div>
				<div class="stimg__middle-sheet">
					<MiddleSheetImg />
				</div>
				<div class="stimg__bottom-sheet">
					<BottomSheetImg />
				</div>

				<div class="stimg__branch">
					<div class="stimg__branch-plus" />
				</div>
			</div>

			<span class="text-base-body-12 new-branch-caption"
				>Drag and drop files to create a new branch</span
			>

			<Button
				color="neutral"
				kind="outlined"
				icon="plus-small"
				on:click={() => branchController.createBranch({})}>New branch</Button
			>
		</div>
	</div>
</div>

<style lang="postcss">
	.canvas-dropzone {
		display: flex;
		height: 100%;
	}

	.new-virtual-branch {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		width: 24rem;
		height: 100%;
		border-radius: var(--radius-m);
		border: 1px dashed color-mix(in srgb, var(--clr-theme-container-outline-pale) 50%, transparent);
		background-color: transparent;
	}

	.new-virtual-branch__content {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: var(--space-16);
	}

	/* ILLUSTRATION */
	.stimg {
		position: relative;
		width: 120px;
		height: 130px;
		opacity: 0.8;

		& div {
			position: absolute;
			background-size: cover;
			background-repeat: no-repeat;
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
	}

	.stimg__middle-sheet {
		z-index: 3;
		top: 42px;
		left: 29px;
	}

	.stimg__bottom-sheet {
		z-index: 2;
		top: 66px;
		left: 50px;
	}

	.stimg__branch {
		z-index: 1;
		top: 0;
		left: 45px;
		width: 66px;
		height: 95px;
		background-color: color-mix(in srgb, var(--clr-core-pop-55) 12%, transparent);
		border-radius: 12px;
	}

	.stimg__branch-plus {
		position: absolute;
		top: 14px;
		left: 50%;
		transform: translateX(-50%);
		width: 34px;
		height: 34px;
		opacity: 0.2;

		&::before,
		&::after {
			content: '';
			position: absolute;
			top: 50%;
			left: 50%;
			transform: translate(-50%, -50%);
			width: 100%;
			height: 2px;
			background-color: var(--clr-theme-scale-pop-40);
		}

		&::after {
			transform: translate(-50%, -50%) rotate(90deg);
		}
	}

	/* OTHER */

	.new-branch-caption {
		color: var(--clr-theme-scale-ntrl-0);
		text-align: center;
		opacity: 0.3;
		max-width: 130px;
	}

	/* DRAGZONE MODIEFIERS */
	.canvas-dropzone {
		&:global(.new-dz-active > .new-virtual-branch) {
			background-color: color-mix(in srgb, var(--clr-theme-scale-ntrl-50) 4%, transparent);
			/* border: 1px dashed color-mix(in srgb, var(--clr-theme-scale-ntrl-50) 100%, transparent); */
			color: var(--clr-theme-scale-pop-50);
		}
	}
</style>
