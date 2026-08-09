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
	getMinimapOverlays,
	getMinimapViewport,
	type MinimapFile,
	type MinimapGeometry,
	type MinimapSelection,
	scrollMinimapTo,
} from "./diff-minimap.ts";
import { type MinimapBadge, paintMinimap } from "./diff-minimap-canvas.ts";
import styles from "./DiffMinimap.module.css";

/**
 * Height the lens keeps however little of the diff the window holds, so it stays
 * a thing you can take hold of rather than a hairline.
 */
const MARKER_MIN_HEIGHT = 14;

const markerHeight = (share: number, track: number): number =>
	Math.min(Math.max(share * track, MARKER_MIN_HEIGHT), track);

/** Track left for the lens to move down, once it has taken its own height out. */
const travel = (track: number, marker: number): number => Math.max(track - marker, 0);

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
	/** Where the pointer took hold of the lens, in pixels from its top. */
	const grabRef = useRef(0);
	const dataRef = useRef({ files, diffStyle, annotationsByPath, selection });
	const geometryRef = useRef<MinimapGeometry | null>(null);
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

		const draw = (): void => {
			const { files, diffStyle, annotationsByPath, selection } = dataRef.current;
			const geometry = getMinimapGeometry(
				viewer,
				files.map((file) => file.itemId),
			);

			geometryRef.current = geometry;
			setNavigable(geometry !== null);
			if (!geometry) return;

			const overlays = getMinimapOverlays({ files, geometry, annotationsByPath, selection });
			setBadges(paintMinimap(canvas, { files, geometry, diffStyle, overlays }));
		};

		const sync = (force: boolean): void => {
			// Item heights start out estimated and firm up as virtualization renders
			// them, which moves every file below. Total height moves with them, so it
			// doubles as a cheap "the layout shifted" check on the scroll path.
			const scrollHeight = viewer.getScrollHeight();
			if (force || scrollHeight !== lastScrollHeight) {
				lastScrollHeight = scrollHeight;
				draw();
			}

			const ruler = rulerRef.current;
			if (!ruler) return;

			// The ruler is collapsed until the first draw finds something to map, and
			// a lens measured against no track at all would be written away to
			// nothing. The draw that gives it a height also resizes the canvas, which
			// brings us straight back here.
			//
			// Measured to the sub-pixel, as the drag is: a track rounded here and not
			// there puts the two a fraction of a percent apart, which on a diff long
			// enough to floor the lens is thousands of lines.
			const track = ruler.getBoundingClientRect().height;
			if (track === 0) return;

			const viewport = getMinimapViewport(viewer);
			const marker = markerHeight(viewport.height, track);
			ruler.style.setProperty(
				"--minimap-marker-top",
				`${viewport.progress * travel(track, marker)}px`,
			);
			ruler.style.setProperty("--minimap-marker-height", `${marker}px`);
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

		return () => {
			resyncRef.current = null;
			if (frame !== null) cancelAnimationFrame(frame);
			unsubscribe();
			resizeObserver.disconnect();
			scheme.removeEventListener("change", redraw);
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

	const fractionAt = (event: PointerEvent<HTMLDivElement>): number | null => {
		const { height, top } = event.currentTarget.getBoundingClientRect();
		if (height === 0) return null;

		// A captured pointer travels outside the ruler, and this is a fraction of
		// it — so keep it one rather than leaving every caller to cope.
		return Math.min(Math.max((event.clientY - top) / height, 0), 1);
	};

	// Placing the lens rather than pointing at content: the pointer carries it by
	// the spot it was taken hold of, over the track its own height leaves.
	const dragTo = (event: PointerEvent<HTMLDivElement>): void => {
		const viewer = viewerRef.current?.getInstance();
		const { height: track, top: trackTop } = event.currentTarget.getBoundingClientRect();
		if (!viewer || track === 0) return;

		const room = travel(track, markerHeight(getMinimapViewport(viewer).height, track));
		const top = event.clientY - trackTop - grabRef.current;

		scrollMinimapTo(viewer, room === 0 ? 0 : Math.min(Math.max(top / room, 0), 1));
	};

	// The label's position is written straight to the DOM, so following the
	// pointer costs no renders; only naming a different file does.
	const describe = (fraction: number): void => {
		const geometry = geometryRef.current;
		const ruler = rulerRef.current;
		if (!geometry || !ruler) return;

		ruler.style.setProperty("--minimap-hint-top", `${fraction * 100}%`);

		const offset = fraction * geometry.contentHeight;
		const index = geometry.blocks.findLastIndex((block) => block.top <= offset);
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
				if (!viewer) return;

				// Taking hold of the lens keeps the point you grabbed, the way a
				// scrollbar does; pressing the track jumps its middle there and drags on
				// from the middle. Its own rect is the hit test, so the minimum height
				// it can shrink to doesn't have to be repeated here.
				const marker = markerRef.current?.getBoundingClientRect();
				const held = marker && event.clientY >= marker.top && event.clientY <= marker.bottom;
				grabRef.current = held
					? event.clientY - marker.top
					: (marker?.height ?? MARKER_MIN_HEIGHT) / 2;

				event.currentTarget.setPointerCapture(event.pointerId);
				// Taking hold shouldn't move anything; pressing the track jumps.
				if (!held) dragTo(event);
			}}
			onPointerMove={(event) => {
				const fraction = fractionAt(event);
				if (fraction === null) return;

				describe(fraction);
				if (event.currentTarget.hasPointerCapture(event.pointerId)) dragTo(event);
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
