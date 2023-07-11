<script lang="ts">
	import type { Bookmark, Delta } from '$lib/api';
	import { derived, Loaded, type Loadable } from 'svelte-loadable-store';
	import type { Readable } from '@square/svelte-store';
	import { ModuleChapters, ModuleMarkers, type Marker } from './slider';
	import { JSR, ModuleSlider } from 'mm-jsr';

	export let sessions: Readable<Loadable<[string, Delta][][]>>;
	export let value: number;
	export let bookmarks: Readable<Loadable<Bookmark[]>>;

	$: bookmarkedTimestamps = derived(bookmarks, (bookmarks) =>
		bookmarks.filter(({ deleted }) => !deleted).map((bookmark) => bookmark.timestampMs)
	);

	$: markers = derived([sessions, bookmarkedTimestamps], ([sessions, bookmarkedTimestamps]) =>
		sessions.flatMap((session, index, all) => {
			const from = all.slice(0, index).reduce((acc, deltas) => acc + deltas.length, 0);
			return session
				.map((delta, index) => ({
					timestampMs: delta[1].timestampMs,
					value: from + index,
					large: false
				}))
				.filter(({ timestampMs }) => bookmarkedTimestamps.includes(timestampMs));
		})
	);

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
		markers: Marker[];
	};

	const jsrSlider = (target: HTMLElement, config: Config) => {
		const fromConfig = (target: HTMLElement, config: Config) => {
			const moduleMarkers = new ModuleMarkers(config.markers);
			const jsr = new JSR({
				modules: [new ModuleSlider(), new ModuleChapters(config.chapters), moduleMarkers],
				config: {
					min: config.min,
					max: config.max,
					step: 1,
					initialValues: [config.initialValue],
					container: target
				}
			});
			jsr.onValueChange(({ real }) => (value = real));
			jsr.onValueChange(({ real }) => {
				config.markers.forEach((marker) => {
					const markerChapter = config.chapters.find(
						([from, to]) => from <= marker.value && marker.value < to
					);
					if (!markerChapter) return;
					const isChapterSelected =
						markerChapter &&
						markerChapter[0] <= real &&
						(real < markerChapter[1] || real === config.max);
					moduleMarkers.setLarge(marker.value, isChapterSelected);
				});
			});
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

{#if !$totalDeltas.isLoading && Loaded.isValue($totalDeltas) && !$chapters.isLoading && Loaded.isValue($chapters) && !$markers.isLoading && Loaded.isValue($markers)}
	<div
		use:jsrSlider={{
			min: 0,
			max: $totalDeltas.value,
			initialValue: value,
			chapters: $chapters.value,
			markers: $markers.value
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

			/* slider */

			.jsr_slider {
				position: absolute;
				left: 0;
				top: 7.5px;

				display: flex;
				align-items: center;

				transform: translate(-50%, -50%);

				width: 16px;
				height: 48px;

				cursor: col-resize;
				transition: background 0.1s ease-in-out;
				z-index: 1;
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

			/* chapters */

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

			/* markers */

			.jsr_marker {
				position: absolute;
				top: 7.5px;
				width: 8px;
				height: 8px;
				left: 0;
				transform: translate(-50%, -50%);
			}

			.jsr_marker::before {
				position: absolute;
				top: 50%;
				left: 50%;
				transform: translate(-50%, -50%);
				content: '';
				height: 4px;
				width: 4px;
				border-radius: 16px;
				background: #d4d4d8;
			}

			.jsr_marker.jsr_marker--large::before {
				width: 8px;
				height: 8px;
			}

			.jsr_marker--after.jsr_marker--large::before,
			.jsr_marker--after::before {
				background: #2563eb;
			}
		</style>
	</div>
{/if}
