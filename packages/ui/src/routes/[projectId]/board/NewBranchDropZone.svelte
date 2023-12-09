<script lang="ts">
	import {
		isDraggableHunk,
		isDraggableFile,
		type DraggableFile,
		type DraggableHunk
	} from '$lib/draggables';
	import { dropzone } from '$lib/utils/draggable';
	import type { BranchController } from '$lib/vbranches/branchController';

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
	<button
		id="new-branch-dz"
		class="new-virtual-branch"
		on:click={() => branchController.createBranch({})}
	>
		<div class="new-virtual-branch__content">
			<svg
				width="20"
				height="20"
				viewBox="0 0 20 20"
				fill="none"
				xmlns="http://www.w3.org/2000/svg"
			>
				<path d="M10 20V0M0 10L20 10" stroke="currentcolor" stroke-width="1.5" />
			</svg>

			<span class="text-base-12 text-semibold">New virtual branch</span>
		</div>
	</button>
</div>

<style lang="postcss">
	.canvas-dropzone {
		display: flex;
		height: 100%;
	}

	.new-virtual-branch {
		color: var(--clr-theme-scale-ntrl-0);
		width: 20rem;
		height: 100%;
		border-radius: var(--radius-m);
		border: 1px dashed var(--clr-theme-container-outline-light);

		&:hover {
			& .new-virtual-branch__content {
				opacity: 0.3;
				transform: translateY(0);

				& span {
					opacity: 1;
				}
			}
		}
	}

	.new-virtual-branch__content {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: var(--space-16);
		opacity: 0.15;
		transform: translateY(calc(var(--space-12)));
		transition:
			transform 0.2s var(--transition-fast),
			opacity 0.2s var(--transition-fast);

		& span {
			opacity: 0;
			transition: opacity 0.2s var(--transition-fast);
		}
	}
</style>
