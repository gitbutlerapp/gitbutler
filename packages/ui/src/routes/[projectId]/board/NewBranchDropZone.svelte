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
				fill="currentcolor"
				xmlns="http://www.w3.org/2000/svg"
			>
				<path
					fill-rule="evenodd"
					clip-rule="evenodd"
					d="M10.75 10.75V20H9.25V10.75H0V9.25H9.25V0H10.75V9.25H20V10.75H10.75Z"
				/>
			</svg>

			<span class="text-base-12 text-semibold" />
		</div>
	</button>
</div>

<style lang="postcss">
	.canvas-dropzone {
		display: flex;
		height: 100%;
		width: 100%;
	}

	.new-virtual-branch {
		color: color-mix(in srgb, var(--clr-theme-scale-ntrl-0) 30%, transparent);
		width: 20rem;
		height: 100%;
		border-radius: var(--radius-m);
		border: 1px solid color-mix(in srgb, var(--clr-theme-container-outline-light) 40%, transparent);
		background-color: transparent;
		transition:
			opacity var(--transition-medium),
			background-color var(--transition-medium),
			border-color var(--transition-medium),
			color var(--transition-medium);

		&:hover {
			opacity: 0.8;
			background-color: color-mix(in srgb, var(--clr-theme-container-sub) 60%, transparent);
			border: 1px solid color-mix(in srgb, var(--clr-theme-container-sub) 60%, transparent);
			color: var(--clr-theme-scale-ntrl-40);

			& .new-virtual-branch__content {
				opacity: 1;
				transform: translateY(0);

				& span {
					opacity: 0.5;
					transform: translateY(0);
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
		transform: translateY(calc(var(--space-20)));
		transition:
			transform var(--transition-medium),
			opacity var(--transition-medium);

		& span {
			opacity: 0;
			transition:
				opacity var(--transition-medium),
				transform var(--transition-medium);
			transform: translateY(calc(var(--space-8) * -1));

			&:after {
				content: 'New Branch';
			}
		}

		& svg {
			transition: fill var(--transition-medium);
		}
	}

	/* DRAGZONE MODIEFIERS */
	.canvas-dropzone {
		&:global(.new-dz-active > .new-virtual-branch) {
			background-color: color-mix(in srgb, var(--clr-theme-pop-container-dark) 30%, transparent);
			border: 1px solid color-mix(in srgb, var(--clr-theme-pop-container-dark) 30%, transparent);
			color: var(--clr-theme-scale-pop-50);

			& .new-virtual-branch__content {
				& span {
					&:after {
						content: 'Drop to create new branch';
					}
				}
			}
		}

		&:global(.new-dz-hover > .new-virtual-branch) {
			background-color: color-mix(in srgb, var(--clr-theme-pop-container-dark) 60%, transparent);
			border: 1px solid color-mix(in srgb, var(--clr-theme-pop-container-dark) 60%, transparent);
			color: var(--clr-theme-scale-pop-50);

			& .new-virtual-branch__content {
				opacity: 1;
				transform: translateY(0);

				& span {
					opacity: 0.6;
					transform: translateY(0);
				}
			}
		}
	}
</style>
