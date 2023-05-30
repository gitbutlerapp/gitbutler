<script lang="ts">
	import type { Bookmark, Delta } from '$lib/api';
	import { derived, Value, type Loadable } from 'svelte-loadable-store';
	import type { Readable } from '@square/svelte-store';
	import { ModuleChapters } from './ModuleChapters';
	import { JSR, ModuleSlider } from 'mm-jsr';

	export let sessions: Readable<Loadable<[string, Delta][][]>>;
	export let value: number;
	export let bookmarks: Loadable<Bookmark[]>;

	$: markers =
		bookmarks.isLoading || Value.isError(bookmarks.value)
			? ({} as Record<number, string>)
			: (Object.fromEntries(
					bookmarks.value
						.filter(({ deleted }) => !deleted)
						.map(({ timestampMs, note }) => [timestampMs, note])
			  ) as Record<number, string>);

	$: totalDeltas = derived(sessions, (sessions) =>
		sessions.reduce((acc, deltas) => acc + deltas.length, 0)
	);

	$: chapters = derived(sessions, (sessions) =>
		sessions.map((session, index, all) => {
			const from = all.slice(0, index).reduce((acc, deltas) => acc + deltas.length, 0);
			const to = from + session.length;
			return [from, to] as [number, number];
		})
	);

	type Config = {
		min: number;
		max: number;
		initialValue: number;
		chapters: [number, number][];
	};

	const jsrSlider = (target: HTMLElement, config: Config) => {
		const fromConfig = (target: HTMLElement, config: Config) => {
			const jsr = new JSR({
				modules: [new ModuleSlider(), new ModuleChapters(config.chapters)],
				config: {
					min: config.min,
					max: config.max,
					step: 1,
					initialValues: [config.initialValue],
					container: target
				}
			});
			jsr.onValueChange(({ real }) => (value = real));
			return jsr;
		};

		let jsr = fromConfig(target, config);
		return {
			update(config: Config) {
				jsr.destroy();
				jsr = fromConfig(target, config);
			},
			destroy() {
				jsr.destroy();
			}
		};
	};
</script>

{#if !$totalDeltas.isLoading && Value.isValue($totalDeltas.value) && !$chapters.isLoading && Value.isValue($chapters.value)}
	<div
		use:jsrSlider={{
			min: 0,
			max: $totalDeltas.value,
			initialValue: value,
			chapters: $chapters.value
		}}
	>
		<style>
			.jsr {
				position: relative;

				height: 100%;
				width: 100%;

				-webkit-user-select: none;
				-moz-user-select: none;
				-ms-user-select: none;
				user-select: none;
			}

			.jsr_slider {
				position: absolute;
				left: 0;
				top: 8px;

				display: flex;
				align-items: center;

				transform: translate(-50%, -50%);

				width: 16px;
				height: 48px;

				cursor: col-resize;
				transition: background 0.1s ease-in-out;
			}

			.jsr_slider::before {
				content: '';
				width: 3px;
				height: 18px;
				position: absolute;
				top: 50%;
				left: 50%;
				transform: translate(-50%, -50%);
				background: white;
				border-radius: 2px;
			}

			.jsr_chapters {
				display: flex;
				align-items: center;
				height: 15px;
			}

			.jsr_chapter {
				display: flex;
				align-items: center;
				height: 6px;
				border-radius: 5px;
				background: var(--color-zinc-700);
			}

			.jsr_chapter__filled,
			.jsr_chapter__not-filled {
				border-radius: 5px;
			}

			.jsr_chapter__filled {
				height: 100%;
				background: #2563eb;
			}

			.jsr_chapter--active > .jsr_chapter__not-filled,
			.jsr_chapter--active > .jsr_chapter__filled {
				border-radius: 8px;
			}

			.jsr_chapter--active {
				height: 10px;
				border-radius: 8px;
			}
		</style>
	</div>
{/if}
