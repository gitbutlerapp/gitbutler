import { createLiteApi } from "#electron/lite-api.ts";
import { tauriTransport } from "./transport.ts";
import "./window-drag.ts";

window.lite = createLiteApi(tauriTransport);

// Imported only once the api exists: hotkeys.ts reads `window.lite.platform` at module scope.
void import("#ui/main.tsx");
