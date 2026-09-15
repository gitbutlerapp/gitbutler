import { createContext } from "react";

// oxlint-disable-next-line typescript/no-non-null-assertion -- Required context.
export const CheckForUpdatesContext = createContext<() => void>(null!);
