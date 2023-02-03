export { attachConsole as setup } from "tauri-plugin-log-api";

import * as log from "tauri-plugin-log-api";

export const debug = (...args: any[]) =>
    log.debug(args.map((argument) => JSON.stringify(argument)).join(" "));

export const info = (...args: any[]) =>
    log.info(args.map((argument) => JSON.stringify(argument)).join(" "));

export const error = (...args: any[]) =>
    log.error(args.map((argument) => JSON.stringify(argument)).join(" "));
