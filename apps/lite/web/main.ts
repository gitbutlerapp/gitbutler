/**
 * Lite in a browser, talking to a but-server. The api must exist before the
 * app's modules load, since some read `window.lite` at module scope.
 */
import { createLiteApi } from "#electron/lite-api.ts";
import { createHttpTransport } from "./http-transport.ts";

const serverUrl = import.meta.env.VITE_BUT_SERVER_URL ?? "http://localhost:6978";

window.lite = createLiteApi(createHttpTransport(serverUrl));

await import("#ui/main.tsx");
