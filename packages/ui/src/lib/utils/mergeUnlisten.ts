type UnlistenFunc = () => void;

/**
 * Helper function to merge multiple unlisten callbacks.
 *
 * @example
 * $effect(()=> {
 *   return mergeUnlisten(
 *     on(document, 'mousedown', onMouseDown),
 *     on(document, 'mouseup', onMouseUp)
 *   );
 * })
 */
export function mergeUnlisten(...callbacks: Array<UnlistenFunc>): () => void {
	return () => {
		for (let i = callbacks.length - 1; i >= 0; i--) {
			callbacks[i]();
		}
		callbacks.length = 0;
	};
}
