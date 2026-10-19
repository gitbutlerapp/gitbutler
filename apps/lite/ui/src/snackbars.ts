import { Toast } from "@base-ui/react";
import type { SnackbarData } from "@gitbutler/ui-react/Snackbars.tsx";

/**
 * Raises snackbars: the one-line news that stacks at the bottom centre, separate from the toasts
 * in the corner. `App` draws them; anything may add one.
 */
export const snackbarManager = Toast.createToastManager<SnackbarData>();
