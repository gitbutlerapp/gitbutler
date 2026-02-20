<script lang="ts">
	import { goto } from '$app/navigation';
	import { showError } from '$lib/notifications/toasts';
	import { handleAddProjectOutcome } from '$lib/project/project';
	import { PROJECTS_SERVICE } from '$lib/project/projectsService';
	import { projectPath } from '$lib/routes/routes.svelte';
	import { inject } from '@gitbutler/core/context';
	import { getCurrentWebview } from '@tauri-apps/api/webview';

	const projectsService = inject(PROJECTS_SERVICE);

	let isDraggingOver = $state(false);
	let isProcessing = $state(false);

	async function handleDrop(paths: string[]) {
		if (isProcessing) return;

		const path = paths[0];
		if (!path) return;

		isProcessing = true;
		isDraggingOver = false;

		try {
			if (!projectsService.validateProjectPath(path)) return;

			const outcome = await projectsService.addProject(path);
			if (outcome) {
				handleAddProjectOutcome(outcome, (project) => goto(projectPath(project.id)));
			}
		} catch (e: unknown) {
			console.error('Failed to add project from drop', e);
			showError('Failed to add project', 'Something went wrong while adding the dropped folder.');
		} finally {
			isProcessing = false;
		}
	}

	$effect(() => {
		const webview = getCurrentWebview();

		const unlistenPromise = webview.onDragDropEvent((event) => {
			const { type } = event.payload;

			if (type === 'enter') {
				isDraggingOver = true;
			} else if (type === 'leave') {
				isDraggingOver = false;
			} else if (type === 'drop') {
				handleDrop(event.payload.paths);
			}
		});

		return () => {
			unlistenPromise.then((unlisten) => unlisten());
		};
	});
</script>

{#if isDraggingOver}
	<div class="drop-overlay">
		<div class="drop-overlay__content">
			<svg width="100%" height="100%" class="animated-rectangle">
				<rect width="100%" height="100%" rx="14" ry="14" vector-effect="non-scaling-stroke" />
			</svg>

			<div class="drop-overlay__icon">
				<svg
					width="104"
					height="99"
					viewBox="0 0 104 99"
					fill="none"
					xmlns="http://www.w3.org/2000/svg"
				>
					<path
						d="M14.3539 41.9059C13.6625 37.6479 16.9496 33.7839 21.2634 33.7839H50.6751C54.108 33.7839 57.0344 36.2734 57.5846 39.662L62.8042 71.806C63.4956 76.064 60.2084 79.9279 55.8947 79.9279H26.483C23.05 79.9279 20.1237 77.4385 19.5735 74.0499L14.3539 41.9059Z"
						fill="var(--clr-theme-pop-soft)"
						stroke="var(--clr-art-spot-fill)"
						stroke-width="1.2"
					/>
					<path
						class="float-a"
						d="M38.8468 20.0029L65.3226 19.1562L76.7668 29.4769L77.5998 55.5256L40.0213 56.7274L38.8468 20.0029Z"
						fill="var(--clr-bg-1)"
						stroke="var(--clr-art-spot-fill)"
						stroke-width="1.2"
					/>
					<g class="float-b">
						<path
							d="M0.674927 5.73694L28.3387 2.00145L45.0855 14.2567L50.0708 51.1757L7.58528 56.9126L0.674927 5.73694Z"
							fill="var(--clr-bg-1)"
						/>
						<path
							d="M28.4479 6.33394L29.9319 17.3236L42.2952 15.6541M0.674927 5.73694L7.58528 56.9126L50.0708 51.1757L45.0855 14.2567L28.3387 2.00145L0.674927 5.73694Z"
							stroke="var(--clr-art-spot-fill)"
							stroke-width="1.2"
						/>
					</g>
					<path
						d="M81.9095 79.9279H25.3949C29.1349 79.9279 30.4949 78.5708 31.1749 74.1599L35.9425 40.8562C36.5234 36.7979 39.9996 33.7839 44.0993 33.7839L56.465 33.7839C59.5218 33.7839 62.3274 35.4761 63.7534 38.1799L65.9165 42.2811C67.3425 44.9849 70.1481 46.6771 73.2049 46.6771H85.3393C90.5536 46.6771 94.4569 51.4594 93.4122 56.5681L89.9825 73.3389C89.1982 77.174 85.824 79.9279 81.9095 79.9279Z"
						fill="var(--clr-bg-1)"
						stroke="var(--clr-art-spot-fill)"
						stroke-width="1.2"
					/>
					<path
						d="M77.3069 2.47192V15.2439M103.675 28.0159H90.4909M97.7432 8.24973L87.0704 18.589"
						stroke="var(--clr-art-spot-fill)"
						stroke-width="1.2"
					/>
					<g>
						<path
							d="M61.5477 61.2045C64.6142 57.6628 69.0442 58.8763 70.2896 61.9801C75.3417 58.7669 77.7802 64.573 77.5737 69.1215C77.3868 73.2376 76.7499 80.2039 73.0737 85.69C74.5926 86.8616 75.2171 88.4864 75.0205 90.0435C74.0836 97.461 52.6294 98.8749 51.516 91.6397C51.325 90.3976 51.6162 89.0919 52.4777 87.9025C44.8091 84.0183 41.8241 77.4965 42.1905 74.5933C42.5787 71.5193 44.7353 70.3875 47.5131 71.2188C42.9869 63.3072 50.3483 59.1353 53.4866 62.805C55.0641 58.0934 59.9802 58.0602 61.5477 61.2045Z"
							fill="var(--clr-bg-1)"
						/>
						<path
							d="M73.0737 85.69C76.7499 80.2039 77.3868 73.2376 77.5737 69.1215C77.7802 64.573 75.3417 58.7669 70.2896 61.9801C69.0442 58.8763 64.6142 57.6628 61.5477 61.2045C59.9802 58.0602 55.0641 58.0934 53.4866 62.805C50.3483 59.1353 42.9869 63.3072 47.5131 71.2188C44.7353 70.3875 42.5787 71.5193 42.1905 74.5933C41.8241 77.4965 44.8091 84.0183 52.4777 87.9025M73.0737 85.69C74.5926 86.8616 75.2171 88.4863 75.0205 90.0435C74.0836 97.461 52.6294 98.8749 51.516 91.6397C51.325 90.3976 51.6162 89.0919 52.4777 87.9025M73.0737 85.69C72.2426 85.0486 71.135 84.6142 69.7711 84.2373M52.4777 87.9025C54.7229 84.7678 59.2645 83.9428 62.8494 83.7019M56.0606 69.3775C56.7751 70.8907 57.2158 73.1085 57.2728 74.5756M63.3339 68.8545C63.736 70.3704 63.7521 72.7143 63.5706 74.1512M70.2289 68.986C70.5226 70.4645 70.1942 73.0657 69.8308 74.4797M57.7622 91.8065C60.3859 90.1611 65.7379 89.5191 68.7544 90.6149"
							stroke="var(--clr-art-spot-fill)"
							stroke-width="1.2"
						/>
					</g>
				</svg>
			</div>

			<p class="clr-text-1 text-16 text-bold">Drop folder to add project</p>
			<p class="clr-text-3 text-14">Drop a Git repository folder to add it as a project</p>
		</div>
	</div>
{/if}

<style lang="postcss">
	.drop-overlay {
		display: flex;
		z-index: 9999;
		position: fixed;
		align-items: center;
		justify-content: center;
		inset: 0;
		backdrop-filter: blur(6px);
		background-image: radial-gradient(circle at center, var(--clr-bg-1) 20%, transparent 100%);
		/* background-image: radial-gradient(circle at center, red 0%, transparent 80%); */
		background-position: center;
		background-size: 200% 200%;
		background-repeat: no-repeat;
		animation: fade-in 0.15s ease-out;
		pointer-events: none;
	}

	.drop-overlay__content {
		display: flex;
		position: relative;
		flex-direction: column;
		align-items: center;
		padding: 60px;
		gap: 12px;
	}

	.drop-overlay__icon {
		display: flex;
		animation: scale-up 0.25s ease-out;
	}

	.animated-rectangle {
		position: absolute;
		width: 100%;
		height: 100%;
		inset: 0;
		overflow: visible;
		pointer-events: none;
	}

	.animated-rectangle rect {
		fill: none;
		stroke: var(--clr-border-2);
		stroke-dasharray: 3 4;
		stroke-width: 1.5;
		animation: dash-march 1.5s linear infinite;
	}

	@keyframes dash-march {
		to {
			stroke-dashoffset: -14;
		}
	}

	.float-a {
		animation: float-a 3s ease-in-out infinite;
	}

	.float-b {
		animation: float-b 3.5s ease-in-out infinite;
	}

	@keyframes float-a {
		0%,
		100% {
			transform: translateY(0);
		}
		50% {
			transform: translateY(-4px);
		}
	}

	@keyframes float-b {
		0%,
		100% {
			transform: translateY(0);
		}
		50% {
			transform: translateY(4px);
		}
	}

	@keyframes scale-up {
		from {
			transform: scale(0.85);
			opacity: 0;
		}
		to {
			transform: scale(1);
			opacity: 1;
		}
	}

	@keyframes fade-in {
		from {
			opacity: 0;
		}
		to {
			opacity: 1;
		}
	}
</style>
