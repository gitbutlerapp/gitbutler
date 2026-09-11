import { reportError } from "#ui/error-reporting.ts";
import type { MouseEvent } from "react";

/**
 * Click handler for anchors whose href must open in the system browser —
 * inside Electron a plain anchor would navigate the app window itself.
 */
export const openLinkExternally = (evt: MouseEvent<HTMLAnchorElement>): void => {
	evt.preventDefault();
	window.lite.openInWebBrowser(evt.currentTarget.href).catch((error: unknown) => {
		reportError(error);
	});
};
