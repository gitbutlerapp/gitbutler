/**
 * Polyfill for AbortSignal.timeout()
 *
 * This method is not available in older WebKit versions (< Safari 16.0 / macOS 12.3).
 * Tauri on macOS may use an older WebKit version that doesn't support this API.
 *
 * @see https://developer.mozilla.org/en-US/docs/Web/API/AbortSignal/timeout_static
 */
export function polyfillAbortSignalTimeout() {
	if (typeof AbortSignal !== 'undefined' && !AbortSignal.timeout) {
		AbortSignal.timeout = function (ms: number): AbortSignal {
			const controller = new AbortController();
			setTimeout(() => {
				controller.abort(
					new DOMException('The operation was aborted due to timeout', 'TimeoutError')
				);
			}, ms);
			return controller.signal;
		};
	}
}
