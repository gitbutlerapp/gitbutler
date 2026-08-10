import type { LocalAnnotationsByPath } from "#ui/annotation.ts";
import { FileIcon } from "#ui/components/FileIcon.tsx";
import type { GUISettings } from "#electron/settings.ts";
import type { CodeViewHandle } from "@pierre/diffs/react";
import {
	type FC,
	type PointerEvent,
	type RefObject,
	useLayoutEffect,
	useRef,
	useState,
} from "react";
import type { Annotation } from "./diff-view.ts";
import {
	getMinimapGeometry,
	getMinimapLayout,
	getMinimapOverlays,
	getMinimapScrollable,
	type MinimapFile,
	type MinimapGeometry,
	type MinimapLayout,
	type MinimapSelection,
	scrollMinimapTo,
} from "./diff-minimap.ts";
import { type MinimapBadge, paintMinimap } from "./diff-minimap-canvas.ts";
import styles from "./DiffMinimap.module.css";

/**
 * Room the ruler has to work in, which is the pane's rather than its own: it
 * ends with the map, and a map scaled against that height would be reasoning in
 * a circle.
 */
const availableTrack = (ruler: HTMLElement): number =>
	ruler.parentElement?.getBoundingClientRect().height ?? 0;

/**
 * A scroll ruler for the diff: every added and removed run drawn where it
 * actually sits in the scroll extent, as wide as its lines are long, with a
 * rule between files.
 *
 * Painted to a canvas from CodeView's live layout rather than rendered from
 * React state, so the file list stays off the scroll path — which matters
 * because the diff panel already re-renders on scroll, as the file under the
 * viewport top drives the selection — and a dense diff costs no DOM.
 */
export const DiffMinimap: FC<{
	viewerRef: RefObject<CodeViewHandle<Annotation> | null>;
	files: Array<MinimapFile>;
	diffStyle: GUISettings["diffStyle"];
	annotationsByPath: LocalAnnotationsByPath;
	selection: MinimapSelection | null;
}> = ({ viewerRef, files, diffStyle, annotationsByPath, selection }) => {
	const rulerRef = useRef<HTMLDivElement>(null);
	const canvasRef = useRef<HTMLCanvasElement>(null);
	const markerRef = useRef<HTMLDivElement>(null);
	/**
	 * The lens as it was when the pointer took hold: where on it the grab landed,
	 * and the mapping to invert. Frozen for the length of the drag because
	 * CodeView's content height is an estimate that firms up as it renders, and a
	 * mapping recomputed under the pointer slides the diff while you hold it.
	 */
	const grabRef = useRef<{ offset: number; layout: MinimapLayout } | null>(null);
	const dataRef = useRef({ files, diffStyle, annotationsByPath, selection });
	const geometryRef = useRef<MinimapGeometry | null>(null);
	const layoutRef = useRef<MinimapLayout | null>(null);
	const resyncRef = useRef<(() => void) | null>(null);
	const [navigable, setNavigable] = useState(false);
	const [hovered, setHovered] = useState<string | null>(null);
	const [badges, setBadges] = useState<Array<MinimapBadge>>([]);

	useLayoutEffect(() => {
		const viewer = viewerRef.current?.getInstance();
		const canvas = canvasRef.current;
		if (!viewer || !canvas) return;

		let frame: number | null = null;
		let forced = false;
		let lastScrollHeight: number | null = null;
		let lastOffset: number | null = null;

		const draw = (layout: MinimapLayout): void => {
			const { files, diffStyle, annotationsByPath, selection } = dataRef.current;
			const geometry = getMinimapGeometry(
				viewer,
				files.map((file) => file.itemId),
			);

			geometryRef.current = geometry;
			setNavigable(geometry !== null);
			if (!geometry) return;

			const overlays = getMinimapOverlays({ files, geometry, annotationsByPath, selection });
			setBadges(paintMinimap(canvas, { files, geometry, layout, diffStyle, overlays }));
		};

		const sync = (force: boolean): void => {
			const ruler = rulerRef.current;
			if (!ruler) return;

			// Measured to the sub-pixel, as the drag is: a track rounded here and not
			// there puts the two a fraction of a percent apart, which at this scale is
			// a good many lines. Zero while the ruler is collapsed, which it is until
			// a draw finds something to map — so the draw has to come first, or it
			// waits on the height that only it can bring about.
			const track = availableTrack(ruler);
			const layout = getMinimapLayout(viewer, track);

			// The ruler stops where the map does, so its own height can't be what the
			// map was scaled against — that would be a box sized from what it holds
			// and holding what it was sized for.
			ruler.style.setProperty("--minimap-track-height", `${Math.min(track, layout.mapHeight)}px`);

			// Item heights start out estimated and firm up as virtualization renders
			// them, which moves every file below. Total height moves with them, so it
			// doubles as a cheap "the layout shifted" check on the scroll path. A map
			// too tall for the ruler also winds under the lens as the diff scrolls,
			// and that is a different picture every time it moves.
			const scrollHeight = viewer.getScrollHeight();
			if (force || scrollHeight !== lastScrollHeight || layout.offset !== lastOffset) {
				lastScrollHeight = scrollHeight;
				lastOffset = layout.offset;
				draw(layout);
			}

			// The canvas the draw just sized is what gives the ruler its height, so
			// the pass that opens it measures nothing and the resize brings us back.
			if (track === 0) return;

			layoutRef.current = layout;
			ruler.style.setProperty("--minimap-marker-top", `${layout.lensTop}px`);
			ruler.style.setProperty("--minimap-marker-height", `${layout.lensHeight}px`);
		};

		// A forced pass has to outlive being folded into a pending scroll one,
		// which would otherwise drop the repaint it was asked for.
		const schedule = (force: boolean) => () => {
			forced ||= force;
			if (frame !== null) return;

			frame = requestAnimationFrame(() => {
				frame = null;
				const repaint = forced;
				forced = false;
				sync(repaint);
			});
		};

		const redraw = schedule(true);
		resyncRef.current = redraw;
		sync(true);

		const unsubscribe = viewer.subscribeToScroll(schedule(false));

		const resizeObserver = new ResizeObserver(redraw);
		resizeObserver.observe(canvas);
		const root = viewer.getContainerElement();
		if (root) {
			resizeObserver.observe(root);
			// CodeView sizes this scaffold to the full content height, so watching it
			// catches folds, annotations and settling measurements — none of which
			// emit a scroll event.
			if (root.firstElementChild) resizeObserver.observe(root.firstElementChild);
		}

		// Canvas colours are sampled, not live CSS, so they need repainting when
		// the tokens behind them resolve differently.
		const scheme = globalThis.matchMedia("(prefers-color-scheme: dark)");
		scheme.addEventListener("change", redraw);

		// The bitmap is sized from the device pixel ratio, which changes with no
		// change to the ruler's own box when the window is dragged onto a display
		// that scales differently — so nothing else here would notice. The query
		// only asks about the ratio it was built from, so re-arm on every change.
		let pixels: MediaQueryList | null = null;
		const watchPixels = (): void => {
			pixels?.removeEventListener("change", onPixels);
			pixels = globalThis.matchMedia(`(resolution: ${globalThis.devicePixelRatio}dppx)`);
			pixels.addEventListener("change", onPixels);
		};
		const onPixels = (): void => {
			watchPixels();
			redraw();
		};
		watchPixels();

		return () => {
			resyncRef.current = null;
			if (frame !== null) cancelAnimationFrame(frame);
			unsubscribe();
			resizeObserver.disconnect();
			scheme.removeEventListener("change", redraw);
			pixels?.removeEventListener("change", onPixels);
		};
	}, [viewerRef]);

	// Republish what the paint loop reads, since it owns its data outside React.
	// Guarded so renders driven by hover don't repaint the canvas.
	useLayoutEffect(() => {
		const previous = dataRef.current;
		const changed =
			previous.files !== files ||
			previous.diffStyle !== diffStyle ||
			previous.annotationsByPath !== annotationsByPath ||
			previous.selection !== selection;
		if (!changed) return;

		dataRef.current = { files, diffStyle, annotationsByPath, selection };
		resyncRef.current?.();
	});

	/** Where the pointer sits on the ruler, in pixels from its top. */
	const rulerOffset = (event: PointerEvent<HTMLDivElement>): number =>
		event.clientY - event.currentTarget.getBoundingClientRect().top;

	/** Bring a place in the content into the middle of the window, and say where that left it. */
	const jumpTo = (content: number): number => {
		const viewer = viewerRef.current?.getInstance();
		if (!viewer) return 0;

		const middle = content - viewer.getHeight() / 2;
		const landing = Math.min(Math.max(middle, 0), getMinimapScrollable(viewer));
		scrollMinimapTo(viewer, landing);

		return landing;
	};

	// Placing the lens rather than pointing at content: the pointer carries it by
	// the spot it was taken hold of, over the room the lens has to move.
	const dragTo = (event: PointerEvent<HTMLDivElement>): void => {
		const viewer = viewerRef.current?.getInstance();
		const grab = grabRef.current;
		if (!viewer || !grab) return;

		const top = rulerOffset(event) - grab.offset;
		const reach = grab.layout.travel === 0 ? 0 : top / grab.layout.travel;
		const progress = Math.min(Math.max(reach, 0), 1);

		// The estimate the mapping was frozen against firms up as the drag scrolls
		// through it, and the far end is the one place that shows: stopping short of
		// a diff that grew under you reads as the lens failing to reach the bottom.
		// So the last of the travel goes wherever the end is now.
		const scrollable = progress === 1 ? getMinimapScrollable(viewer) : grab.layout.scrollable;

		scrollMinimapTo(viewer, progress * scrollable);
	};

	// The label's position is written straight to the DOM, so following the
	// pointer costs no renders; only naming a different file does.
	const describe = (event: PointerEvent<HTMLDivElement>): void => {
		const geometry = geometryRef.current;
		const layout = layoutRef.current;
		const ruler = rulerRef.current;
		if (!geometry || !layout || !ruler) return;

		const y = rulerOffset(event);
		ruler.style.setProperty("--minimap-hint-top", `${y}px`);

		// Back through the drawing: the ruler shows the map from `offset` down, at
		// a fixed scale, so this is the content the pointer is actually over.
		const content = (y + layout.offset) / layout.scale;
		const index = geometry.blocks.findLastIndex((block) => block.top <= content);
		const path = dataRef.current.files[index]?.path ?? null;
		if (path !== hovered) setHovered(path);
	};

	return (
		<div
			ref={rulerRef}
			// The files tree and the diff hotkeys are the accessible route through the
			// same content; this is a pointer shortcut over it.
			aria-hidden
			className={styles.minimap}
			// Read by the diff panel's CSS to drop the native scrollbar only while
			// this is standing in for it.
			data-minimap-navigable={navigable}
			onPointerDown={(event) => {
				event.preventDefault();

				const viewer = viewerRef.current?.getInstance();
				const layout = layoutRef.current;
				if (!viewer || !layout) return;

				event.currentTarget.setPointerCapture(event.pointerId);

				// Taking hold of the lens keeps the point you grabbed, the way a
				// scrollbar does, and moves nothing until you do. Its own rect is the hit
				// test, so the minimum height it can shrink to doesn't have to be
				// repeated here.
				const marker = markerRef.current?.getBoundingClientRect();
				if (marker && event.clientY >= marker.top && event.clientY <= marker.bottom) {
					grabRef.current = { offset: event.clientY - marker.top, layout };
					return;
				}

				// Pressing the track is pointing at the code drawn there, not at a share
				// of the whole diff: the map is a picture at a fixed scale, so read the
				// position back out of it the way the hint does and bring it into the
				// middle of the window.
				const landing = jumpTo((rulerOffset(event) + layout.offset) / layout.scale);

				// The map may have wound on under the pointer, taking the lens with it,
				// so the drag carries on from where the lens has landed. Worked out from
				// the landing rather than read back off the viewer, which goes on
				// reporting the old position for a frame yet — long enough for the first
				// move to teleport by the difference.
				const landed = getMinimapLayout(viewer, availableTrack(event.currentTarget), landing);
				grabRef.current = { offset: rulerOffset(event) - landed.lensTop, layout: landed };
			}}
			onPointerMove={(event) => {
				describe(event);
				if (event.currentTarget.hasPointerCapture(event.pointerId)) dragTo(event);
			}}
			onPointerUp={() => {
				grabRef.current = null;
			}}
			onPointerLeave={() => setHovered(null)}
		>
			<canvas ref={canvasRef} className={styles.canvas} />
			<div ref={markerRef} className={styles.marker} />

			{badges.map(({ index, top }) => {
				const file = files[index];

				return (
					file !== undefined && (
						<FileIcon
							key={file.path}
							fileName={file.path}
							className={styles.badge}
							style={{ top: `${top}px` }}
							// Pressing the track puts the point you pressed in the middle of
							// the viewport; a badge is a file rather than a position, so it
							// opens that file at the top instead. Kept off the ruler's own
							// handler so it doesn't also start a drag.
							onPointerDown={(event) => {
								event.stopPropagation();
								viewerRef.current?.getInstance()?.scrollTo({
									type: "item",
									id: file.itemId,
									align: "start",
									behavior: "instant",
								});
							}}
						/>
					)
				);
			})}

			{hovered !== null && <MinimapHint path={hovered} />}
		</div>
	);
};

const MinimapHint: FC<{ path: string }> = ({ path }) => {
	const separator = path.lastIndexOf("/");
	const name = separator === -1 ? path : path.slice(separator + 1);

	return (
		<div className={styles.hint}>
			<FileIcon fileName={name} className={styles.hintIcon} />
			<span className={styles.hintName}>{name}</span>
			{separator !== -1 && <span className={styles.hintDirectory}>{path.slice(0, separator)}</span>}
		</div>
	);
};
